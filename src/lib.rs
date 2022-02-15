use std::collections::HashMap;
use std::fmt::Debug;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use chrono::prelude::*;
use color_eyre::eyre::{self, WrapErr};
use octocrab::models::ReviewId;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{event, info, instrument, span, warn, Level};
use tracing_subscriber::registry::SpanData;

pub mod api;
pub mod bonusly;
mod config;
mod credentials;
pub mod github;
pub use config::*;
pub use credentials::*;

pub struct Program {
    credentials: Credentials,
    config: Config,
    state: State,
}

// no way this can work right? XXX FIXME TODO
impl Drop for Program {
    fn drop(&mut self) {
        self.state.write_to_path(&self.config.state_path());
    }
}

impl Program {
    #[instrument]
    pub async fn from_config_path(config_path: PathBuf) -> eyre::Result<Self> {
        info!(?config_path, "Reading configuration");
        let mut config: Config = toml::de::from_str(
            &fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config from {config_path:?}"))?,
        )?;
        config.path = config_path.clone();
        let config_parent = config_path
            .parent()
            .ok_or_else(|| eyre::eyre!("Path has no parent: {config_path:?}"))?;

        let credentials_path = config_parent.join(&config.credentials_path);
        info!(?credentials_path, "Reading credentials");
        let credentials: Credentials =
            toml::de::from_str(&fs::read_to_string(&credentials_path).with_context(|| {
                format!("Failed to read credentials from {credentials_path:?}")
            })?)?;

        let state_path = &config_parent.join(&config.data_path);
        info!(?state_path, "Reading state");
        let state = State::from_data_path(state_path, &credentials, &config)
            .await
            .with_context(|| format!("Failed to read state from {state_path:?}"))?;
        Ok(Self {
            config,
            credentials,
            state,
        })
    }

    pub async fn new_approved_reviews(
        &self,
    ) -> eyre::Result<HashMap<github::PullRequest, Vec<octocrab::models::pulls::Review>>> {
        let mut ret = HashMap::new();
        let updated_prs = self
            .config
            .github
            .prs_since(&self.credentials.github, &self.state.cutoff)
            .await?;
        for pr in updated_prs.items {
            let (org, repo) = github::org_repo(&pr).ok_or_else(|| {
                eyre::eyre!("Couldn't parse org/repo from url {}", &pr.repository_url)
            })?;

            let reviews = self
                .credentials
                .github
                .pulls(org, repo)
                // why does this api use different types for pr numbers and pr ids
                // and then use the wrong one
                .list_reviews(pr.number.try_into().unwrap())
                .await?;

            let approved_reviews = reviews
                .items
                .into_iter()
                .filter(|review| {
                    matches!(
                        review.state,
                        Some(octocrab::models::pulls::ReviewState::Approved)
                    ) && !self.state.replied_prs.contains(&review.id)
                })
                .collect::<Vec<_>>();

            if !approved_reviews.is_empty() {
                ret.insert(
                    github::PullRequest {
                        org: org.to_owned(),
                        repo: repo.to_owned(),
                        number: pr.number,
                    },
                    approved_reviews,
                );
            }
        }
        Ok(ret)
    }

    pub async fn reviews(&self) -> eyre::Result<Vec<ReviewStatus>> {
        let mut rng = rand::thread_rng();
        let mut ret = Vec::new();

        // TODO:
        //  - figure out tracking which prs have been replied to or not
        //  - reserve prs with no email on a queue for manual attention later
        //  - think about error handling, particularly re: the state file
        //  - investigate cool parallel shit here with rayon

        for (pr, reviews) in self.new_approved_reviews().await? {
            for review in reviews {
                let user =
                    github::User::from_login(&self.credentials.github, &review.user.login).await?;
                let email = self
                    .config
                    .find_bonusly_email(&self.state.bonusly_users, &user);

                ret.push(match email {
                    Some(email) => ReviewStatus::Ok(bonusly::Bonus {
                        giver_email: self.state.my_bonusly_email.clone(),
                        receiver_email: email,
                        amount: self.config.cherries_per_check,
                        hashtag: self.state.hashtags[rng.gen_range(0..self.state.hashtags.len())]
                            .clone(),
                        reason: format!("thanks for approving my PR! {}", review.html_url),
                    }),
                    None => ReviewStatus::MissingEmail({
                        MissingEmail {
                            org: pr.org.clone(),
                            repo: pr.repo.clone(),
                            pr_number: pr.number,
                            reviewer: review.user.login,
                            id: review.id,
                        }
                    }),
                });
            }
        }
        Ok(ret)
    }

    pub async fn reply(&mut self, review: ReviewStatus) -> eyre::Result<()> {
        match review {
            ReviewStatus::Ok(bonus) => {
                self.credentials.bonusly.send_bonus(&bonus).await?;
            }
            ReviewStatus::MissingEmail(missing_email) => {
                self.state.missing_email.insert(missing_email);
            }
        }

        Ok(())
    }
}

/// Program state. Deserialized from data dir.
#[derive(Serialize, Deserialize, Clone)]
pub struct State {
    replied_prs: HashSet<octocrab::models::ReviewId>,
    missing_email: HashSet<MissingEmail>,
    /// "Don't look for PRs before this datetime"
    cutoff: DateTime<Utc>,
    bonusly_users: Vec<bonusly::User>,
    github_members: Vec<github::User>,
    my_bonusly_email: String,
    hashtags: Vec<String>,
}

impl State {
    #[instrument(skip_all)]
    pub async fn new(credentials: &Credentials, config: &Config) -> eyre::Result<Self> {
        let mut ret = Self {
            cutoff: Utc::now(),
            replied_prs: Default::default(),
            bonusly_users: Default::default(),
            github_members: Default::default(),
            my_bonusly_email: Default::default(),
            hashtags: Default::default(),
            missing_email: Default::default(),
        };
        ret.update(credentials, config).await?;
        Ok(ret)
    }

    #[instrument(skip_all)]
    pub async fn update(&mut self, credentials: &Credentials, config: &Config) -> eyre::Result<()> {
        self.my_bonusly_email = credentials.bonusly.my_email().await?;
        self.hashtags = credentials.bonusly.hashtags().await?;
        self.bonusly_users = credentials.bonusly.list_users().await?;
        self.github_members = credentials
            .github
            .get(format!("orgs/{}/members", config.github.org), None::<&()>)
            .await?;
        Ok(())
    }

    #[instrument(skip(credentials, config))]
    pub async fn from_data_path(
        data_path: &Path,
        credentials: &Credentials,
        config: &Config,
    ) -> eyre::Result<Self> {
        if let Some(parent) = data_path.parent() {
            info!(?parent, "Ensuring state parent dir exists");
            fs::create_dir_all(parent)?;
        }
        if !data_path.exists() {
            info!(?data_path, "State file not found, creating default");
            let new = Self::new(credentials, config).await?;
            serde_json::to_writer(BufWriter::new(File::create(data_path)?), &new)?;
            Ok(new)
        } else {
            Ok(serde_json::from_reader(BufReader::new(File::open(
                data_path,
            )?))?)
        }
    }

    pub async fn write_to_path(&self, data_path: &Path) -> eyre::Result<()> {
        serde_json::to_writer_pretty(BufWriter::new(File::create(data_path)?), &self)?;
        Ok(())
    }
}

pub enum ReviewStatus {
    Ok(bonusly::Bonus),
    MissingEmail(MissingEmail),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MissingEmail {
    org: String,
    repo: String,
    pr_number: i64,
    // GitHub username
    reviewer: String,
    id: ReviewId,
}
