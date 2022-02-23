use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use tracing::info;
use tracing::instrument;

use crate::api;
use crate::bonusly;
use crate::github;

const SECONDS_PER_MINUTE: u64 = 60;

const NAME_REPLACEMENTS: [(&str, &str); 1] = [("Matthew", "Matt")];

#[derive(Clone)]
pub struct Config {
    pub path: PathBuf,
    pub github: github::Config,
    pub cherries_per_check: usize,
    pub state_path: PathBuf,
    pub credentials_path: PathBuf,
    pub pr_check_interval: Duration,
    pub state_update_interval: chrono::Duration,
    pub send_bonus_interval: Duration,
    pub notify_send_user: Option<String>,
}

impl Config {
    #[instrument(level = "debug")]
    pub fn from_path(path: PathBuf) -> eyre::Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize {path:?}"))?;
        let config_parent = path
            .parent()
            .ok_or_else(|| eyre::eyre!("Path has no parent: {path:?}"))?
            .to_path_buf();
        info!(?path, "Reading configuration");
        let config: api::Config = toml::de::from_str(
            &fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config from {path:?}"))?,
        )?;

        Ok(Self {
            path,
            github: config.github,
            cherries_per_check: config.cherries_per_check,
            state_path: config_parent.join(config.data_path),
            credentials_path: config_parent.join(config.credentials_path),
            pr_check_interval: Duration::from_secs(config.pr_check_minutes * SECONDS_PER_MINUTE),
            state_update_interval: chrono::Duration::days(config.state_update_days),
            send_bonus_interval: Duration::from_secs(config.send_bonus_delay_seconds),
            notify_send_user: config.notify_send_user,
        })
    }

    /// Find the bonusly email for a given GitHub user.
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
                // N.b.: no bonusly users in the current data have
                // `full_name != display_name`.
                if &user.full_name == name || &user.display_name == name {
                    return Some(user.email.clone());
                }

                // Try a prefix match, for e.g. "Justin Wood (Callek)"
                if !user.full_name.is_empty() && name.starts_with(&user.full_name) {
                    return Some(user.email.clone());
                }

                // Try replacing e.g. "Matthew" with "Matt" to see if we get a
                // match.
                for (needle, haystack) in NAME_REPLACEMENTS {
                    let replaced = user.full_name.replace(needle, haystack);
                    if &replaced == name {
                        return Some(user.email.clone());
                    }
                }
            }
        }
        None
    }
}
