use std::path::PathBuf;

use serde::Deserialize;

use crate::bonusly;
use crate::github;

#[derive(Deserialize, Clone)]
pub struct Config {
    // TODO: UH OH serde
    #[serde(default = "config_path_default")]
    pub path: PathBuf,

    pub github: github::Config,
    #[serde(default = "cherries_per_check_default")]
    pub cherries_per_check: usize,
    #[serde(default = "data_path_default")]
    pub data_path: PathBuf,
    #[serde(default = "credentials_path_default")]
    pub credentials_path: PathBuf,
}

fn config_path_default() -> PathBuf {
    // uh oh
    "".into()
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
            if let Some(name) = &find.name {
                if &user.full_name == name || &user.display_name == name {
                    return Some(user.email.clone());
                }
            }
        }
        None
    }

    // TODO Probably just store the resolved version
    pub fn state_path(&self) -> PathBuf {
        self.path.join(&self.data_path)
    }
}
