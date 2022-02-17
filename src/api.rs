use std::convert::TryFrom;
use std::path::PathBuf;

use color_eyre::eyre;
use serde::Deserialize;

use crate::github;

#[derive(Deserialize)]
pub struct Credentials {
    bonusly: String,
    github: String,
}

impl TryFrom<Credentials> for super::Credentials {
    type Error = eyre::Error;

    fn try_from(value: Credentials) -> Result<Self, Self::Error> {
        Ok(Self {
            bonusly: super::bonusly::Client::from_token(value.bonusly),
            github: octocrab::Octocrab::builder()
                .personal_token(value.github)
                .build()?,
        })
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub github: github::Config,
    #[serde(default = "cherries_per_check_default")]
    pub cherries_per_check: usize,
    #[serde(default = "data_path_default")]
    pub data_path: PathBuf,
    #[serde(default = "credentials_path_default")]
    pub credentials_path: PathBuf,
    #[serde(default = "pr_check_minutes_default")]
    pub pr_check_minutes: u64,
    #[serde(default = "state_update_days_default")]
    pub state_update_days: i64,
    #[serde(default = "send_bonus_delay_seconds_default")]
    pub send_bonus_delay_seconds: u64,
    #[serde(default)]
    pub notify_send_user: Option<String>,
}

fn send_bonus_delay_seconds_default() -> u64 {
    60
}

fn state_update_days_default() -> i64 {
    7
}

fn pr_check_minutes_default() -> u64 {
    15
}

fn cherries_per_check_default() -> usize {
    1
}

fn credentials_path_default() -> PathBuf {
    "credentials.toml".into()
}

fn data_path_default() -> PathBuf {
    "/var/lib/cherries-4-prs".into()
}
