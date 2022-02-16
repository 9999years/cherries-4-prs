use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;
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
use tracing::{error, info, instrument};

pub mod api;
pub mod bonusly;
mod config;
mod credentials;
pub mod github;
pub use config::*;
pub use credentials::*;

pub struct Program {
    pub credentials: Credentials,
    pub config: Config,
    pub state: State,
}

impl Program {
    #[instrument]
    pub async fn from_config_path(config_path: PathBuf) -> eyre::Result<Self> {
        info!(?config_path, "Reading configuration");
        let config = Config::from_path(config_path.clone())
            .with_context(|| format!("Failed to read config from {config_path:?}"))?;

        let credentials_path = &config.credentials_path;
        info!(?credentials_path, "Reading credentials");
        let credentials: Credentials =
            toml::de::from_str(&fs::read_to_string(credentials_path).with_context(|| {
                format!("Failed to read credentials from {credentials_path:?}")
            })?)?;

        let state_path = &config.state_path;
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

    #[instrument(skip_all)]
    pub async fn write_state(&self) -> eyre::Result<()> {
        self.state.write_to_path(&self.config.state_path).await?;
        Ok(())
    }

    #[instrument(skip_all)]
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
                    ) && !self.state.replied_prs.contains(&RepliedReview {
                        pr: github::PullRequest {
                            org: org.to_owned(),
                            repo: repo.to_owned(),
                            number: pr.number,
                        },
                        reviewer: review.user.login.clone(),
                    })
                })
                .inspect(|review| {
                    info!(
                        ?org,
                        ?repo,
                        pr_number = pr.number,
                        reviewer = %review.user.login,
                        review_id = %review.id,
                        "Found approved review"
                    );
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

        for MissingEmail {
            pr,
            reviewer: _reviewer,
            id,
        } in &self.state.non_replied_prs
        {
            let github::PullRequest { org, repo, number } = pr;
            let review: octocrab::models::pulls::Review = self
                .credentials
                .github
                .get(
                    format!("/repos/{org}/{repo}/pulls/{number}/reviews/{id}"),
                    None::<&()>,
                )
                .await?;
            info!(
                ?org,
                ?repo,
                pr_number = pr.number,
                reviewer = %review.user.login,
                review_id = %review.id,
                "Found review missing email"
            );
            ret.entry(github::PullRequest {
                org: org.clone(),
                repo: repo.clone(),
                number: *number,
            })
            .or_default()
            .push(review);
        }

        Ok(ret)
    }

    #[instrument(skip_all)]
    pub async fn reviews(&mut self) -> eyre::Result<Vec<ReviewStatus>> {
        let mut rng = rand::thread_rng();
        let mut ret = Vec::new();

        // TODO:
        //  - figure out tracking which prs have been replied to or not
        //  - think about error handling, particularly re: the state file
        //  - investigate cool parallel shit here with rayon

        for (pr, reviews) in self.new_approved_reviews().await? {
            for review in reviews {
                let user = self
                    .state
                    .github_user(review.user.login.clone(), &self.credentials)
                    .await?;
                let email = self
                    .config
                    .find_bonusly_email(&self.state.bonusly_users, &user);

                let missing_email = MissingEmail {
                    pr: github::PullRequest {
                        org: pr.org.clone(),
                        repo: pr.repo.clone(),
                        number: pr.number,
                    },
                    reviewer: review.user.login,
                    id: review.id,
                };

                match email {
                    Some(email) => {
                        info!(review = ?missing_email, "Found email for review");
                        self.state.non_replied_prs.remove(&missing_email);
                        ret.push(ReviewStatus::Ok(
                            missing_email,
                            bonusly::Bonus {
                                receiver_email: email,
                                amount: self.config.cherries_per_check,
                                hashtag: self.state.hashtags
                                    [rng.gen_range(0..self.state.hashtags.len())]
                                .clone(),
                                reason: format!("thanks for approving my PR! {}", review.html_url),
                            },
                        ))
                    }
                    None => {
                        if !self.state.non_replied_prs.contains(&missing_email) {
                            info!(?missing_email, "Missing email for review");
                            ret.push(ReviewStatus::MissingEmail(missing_email));
                        }
                    }
                }
            }
        }
        Ok(ret)
    }

    #[instrument(skip(self))]
    pub async fn reply(&mut self, review: ReviewStatus) -> eyre::Result<()> {
        match review {
            ReviewStatus::Ok(missing_email, bonus) => {
                let result = self.credentials.bonusly.send_bonus(&bonus).await;
                tokio::time::sleep(self.config.send_bonus_interval).await;
                match result {
                    Ok(reply) => {
                        info!(?reply, "Sent cherries");
                        self.state.replied_prs.insert(missing_email.into());
                    }
                    Err(err) => {
                        info!(?err, "Failed to send bonus");
                        self.state.non_replied_prs.insert(missing_email);
                        return Err(err);
                    }
                }
            }
            ReviewStatus::MissingEmail(missing_email) => {
                self.state.non_replied_prs.insert(missing_email);
            }
        }

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn reply_all_and_wait(&mut self) -> eyre::Result<()> {
        let reviews = self.reviews().await?;
        info!(?reviews, "Sending cherries for reviews");
        let mut errors = Vec::with_capacity(reviews.len());
        for review in reviews {
            if let Err(err) = self.reply(review).await {
                error!("Error while sending cherries: {:?}", err);
                errors.push(err);
            }

            let duration = Duration::from_secs(10);
            info!("Sleeping {:?} before sending next bonus", duration);
            tokio::time::sleep(duration).await;
        }

        let result = self
            .state
            .maybe_update(&self.credentials, &self.config)
            .await;
        self.state.cutoff = Utc::now();
        self.write_state().await?;
        result?;

        info!("Sleeping for {:?}", self.config.pr_check_interval);
        tokio::time::sleep(self.config.pr_check_interval).await;

        if errors.is_empty() {
            Ok(())
        } else {
            // TODO this is probably really ugly formatting
            Err(eyre::eyre!("{:?}", errors))
        }
    }
}

/// Program state. Deserialized from data dir.
#[derive(Serialize, Deserialize, Clone)]
pub struct State {
    last_update: DateTime<Utc>,
    /// PR-reviewer combos we've already replied to; don't send cherries more
    /// than once per reviewer per PR.
    replied_prs: HashSet<RepliedReview>,
    /// Reviews we haven't replied to; missing emails or API errors.
    non_replied_prs: HashSet<MissingEmail>,
    /// "Don't look for PRs before this datetime"
    cutoff: DateTime<Utc>,
    /// All bonusly users, for correlation with GitHub users.
    bonusly_users: Vec<bonusly::User>,
    /// Map from GitHub username to user info.
    github_members: HashMap<String, github::User>,
    /// Bonusly hashtags.
    hashtags: Vec<String>,
}

impl State {
    #[instrument(skip_all)]
    pub async fn new(credentials: &Credentials, config: &Config) -> eyre::Result<Self> {
        let mut ret = Self {
            cutoff: Utc::now() - chrono::Duration::from_std(config.pr_check_interval).unwrap(),
            last_update: Utc::now(),
            replied_prs: Default::default(),
            bonusly_users: Default::default(),
            github_members: Default::default(),
            hashtags: Default::default(),
            non_replied_prs: Default::default(),
        };
        ret.update(credentials, config).await?;
        Ok(ret)
    }

    #[instrument(skip_all)]
    pub async fn update(
        &mut self,
        credentials: &Credentials,
        _config: &Config,
    ) -> eyre::Result<()> {
        self.last_update = Utc::now();
        self.hashtags = credentials.bonusly.hashtags().await?;
        self.bonusly_users = credentials.bonusly.list_users().await?;
        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn maybe_update(
        &mut self,
        credentials: &Credentials,
        config: &Config,
    ) -> eyre::Result<()> {
        let now = Utc::now();
        if self.last_update + config.state_update_interval <= now {
            self.update(credentials, config).await?;
        }
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

    #[instrument(skip(self, credentials))]
    pub async fn github_user(
        &mut self,
        login: String,
        credentials: &Credentials,
    ) -> eyre::Result<github::User> {
        let maybe_user = self.github_members.get(&login);
        match maybe_user {
            Some(user) => Ok(user.clone()),
            None => {
                let user = github::User::from_login(&credentials.github, &login).await?;
                self.github_members.insert(login.clone(), user);
                // TODO there has got to be a better way to do this
                Ok(self.github_members.get(&login).unwrap().clone())
            }
        }
    }
}

#[derive(Debug)]
pub enum ReviewStatus {
    Ok(MissingEmail, bonusly::Bonus),
    MissingEmail(MissingEmail),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MissingEmail {
    pr: github::PullRequest,
    // GitHub username
    reviewer: String,
    id: ReviewId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RepliedReview {
    pr: github::PullRequest,
    // GitHub username
    reviewer: String,
}

impl From<MissingEmail> for RepliedReview {
    fn from(missing_email: MissingEmail) -> Self {
        Self {
            pr: missing_email.pr,
            reviewer: missing_email.reviewer,
        }
    }
}
