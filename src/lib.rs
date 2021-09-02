#![allow(unused_imports)]

use color_eyre::eyre::{self, WrapErr};
use tracing::{event, info, instrument, span, warn, Level};

pub mod bonusly;
pub mod github;

pub fn find_bonusly_email(users: &[bonusly::User], find: &github::User) -> Option<String> {
    if let Some(email) = &find.email {
        if email.ends_with("@starry.com") {
            return Some(email.clone());
        }
    }
    for user in users {
        if user.full_name == find.name || user.display_name == find.name {
            return Some(user.email.clone());
        }
    }
    None
}
