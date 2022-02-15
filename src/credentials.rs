use serde::Deserialize;

use crate::api;
use crate::bonusly;

#[derive(Deserialize)]
#[serde(try_from = "api::Credentials")]
pub struct Credentials {
    pub bonusly: bonusly::Client,
    pub github: octocrab::Octocrab,
}
