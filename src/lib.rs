#![allow(unused_imports)]

use color_eyre::eyre::{self, WrapErr};
use serde::Deserialize;
use tracing::{event, info, instrument, span, warn, Level};

pub mod bonusly;
pub mod github;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub github: github::Config,
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
