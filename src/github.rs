use octocrab::{models::issues::Issue, Octocrab, Page};
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct User {
    pub id: u64,
    pub login: String,
    pub email: Option<String>,
    pub name: String,
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub user: String,
    pub org: String,
}

impl Config {
    pub async fn prs_since(
        &self,
        github: &Octocrab,
        datetime: &str,
    ) -> Result<Page<Issue>, octocrab::Error> {
        github
            .search()
            .issues_and_pull_requests(&format!(
                "is:pr author:{} review:approved org:{} updated:>={}",
                self.user, self.org, datetime
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
