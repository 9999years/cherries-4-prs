use std::convert::TryFrom;

use color_eyre::eyre;
use serde::Deserialize;

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
