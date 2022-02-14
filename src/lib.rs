#![allow(unused_imports)]

use std::{
    collections::HashSet,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use chrono::prelude::*;
use color_eyre::eyre::{self, WrapErr};
use serde::Deserialize;
use tracing::{event, info, instrument, span, warn, Level};

pub mod bonusly;
pub mod github;

#[derive(Deserialize, Clone)]
pub struct Credentials {
    pub bonusly: String,
    pub github: String,
}

/// Program state. Deserialized from data dir.
#[derive(Deserialize, Clone)]
pub struct State {
    replied_prs: HashSet<octocrab::models::ReviewId>,
    /// "Don't look for PRs before this datetime"
    cutoff: DateTime<Utc>,
    bonusly_users: Vec<bonusly::User>,
    github_members: Vec<github::User>,
}

impl State {
    pub async fn new(credentials: &Credentials, config: &Config) -> eyre::Result<Self> {
        let bonusly_client = bonusly::Client::from_token(credentials.bonusly.clone());

        let github_client = octocrab::Octocrab::builder()
            .personal_token(credentials.github.clone())
            .build()?;

        Ok(Self {
            replied_prs: Default::default(),
            cutoff: Utc::now(),
            bonusly_users: bonusly_client.list_users().await?,
            github_members: github_client
                .get(format!("orgs/{}/members", config.github.org), None::<&()>)
                .await?,
        })
    }

    pub fn from_data_dir(path: impl AsRef<Path>) -> eyre::Result<Self> {
        fs::create_dir_all(&path)?;
        let state_path: PathBuf = [path.as_ref(), Path::new("state.json")].iter().collect();
        Ok(serde_json::from_reader(BufReader::new(File::open(
            state_path,
        )?))?)
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub github: github::Config,
    #[serde(default = "cherries_per_check_default")]
    pub cherries_per_check: usize,
    pub data_dir: PathBuf,
}

fn cherries_per_check_default() -> usize {
    1
}

impl Config {
    pub fn find_bonusly_email(
        &self,
        users: &[bonusly::User],
        find: &github::User,
    ) -> Option<String> {
        // First check for overrides.
        if let Some(email) = self.github.emails.get(&find.login) {
            return Some(email.clone());
        }

        // If find's GitHub profile lists an `@starry.com` email, use it.
        if let Some(email) = &find.email {
            if email.ends_with("@starry.com") {
                return Some(email.clone());
            }
        }
        // Otherwise, use full names / display names.
        for user in users {
            if user.full_name == find.name || user.display_name == find.name {
                return Some(user.email.clone());
            }
        }
        None
    }
}
