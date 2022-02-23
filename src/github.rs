use std::collections::HashMap;
use std::fmt::Display;

use chrono::{DateTime, Utc};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};

pub use octocrab::models::issues::Issue;
pub use octocrab::models::pulls::Review;
pub use octocrab::models::pulls::ReviewState;
pub use octocrab::models::ReviewId;
pub use octocrab::Page;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    pub id: u64,
    pub login: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

impl User {
    pub async fn from_login(github: &Octocrab, login: &str) -> Result<Self, octocrab::Error> {
        github.get(format!("users/{}", login), None::<&()>).await
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub user: String,
    pub org: String,
    /// Map from GitHub usernames to Starry emails
    #[serde(default)]
    pub emails: HashMap<String, String>,
}

impl Config {
    pub async fn prs_since(
        &self,
        github: &Octocrab,
        datetime: &DateTime<Utc>,
    ) -> Result<Page<Issue>, octocrab::Error> {
        github
            .search()
            .issues_and_pull_requests(&format!(
                "is:pr author:{} review:approved org:{} updated:>={}",
                self.user,
                self.org,
                datetime.to_rfc3339()
            ))
            .send()
            .await
    }
}

pub fn org_repo(pr: &Issue) -> Option<(&str, &str)> {
    let mut segments = pr.repository_url.path_segments()?;
    segments.next(); // "repo"
    Some((segments.next()?, segments.next()?))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PullRequest {
    pub org: String,
    pub repo: String,
    pub number: i64,
}

impl Display for PullRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}#{}", self.org, self.repo, self.number)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NonRepliedReview {
    pub pr: PullRequest,
    // GitHub username
    pub reviewer: String,
    pub id: ReviewId,
}

impl Display for NonRepliedReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} reviewed by {} (ID {})",
            self.pr, self.reviewer, self.id
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RepliedReview {
    pub pr: PullRequest,
    // GitHub username
    pub reviewer: String,
}

impl Display for RepliedReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} reviewed by {}", self.pr, self.reviewer)
    }
}

impl From<NonRepliedReview> for RepliedReview {
    fn from(missing_email: NonRepliedReview) -> Self {
        Self {
            pr: missing_email.pr,
            reviewer: missing_email.reviewer,
        }
    }
}
