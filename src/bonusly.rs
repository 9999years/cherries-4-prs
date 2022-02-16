//! See <https://bonusly.docs.apiary.io/>
use std::time::Duration;

use color_eyre::eyre::{self, Report, WrapErr};
use reqwest::{Client as HttpClient, RequestBuilder};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::instrument;

static BONUSLY_API_URL: &str = "https://bonus.ly/api/v1";

/// A Bonusly client. See [`Client::from_token`].
pub struct Client {
    token: SecretString,
    client: HttpClient,
}

impl Client {
    /// Construct a client from a token.
    pub fn from_token(token: String) -> Self {
        Client {
            token: SecretString::from(token),
            client: HttpClient::new(),
        }
    }

    fn request(&self, method: reqwest::Method, endpoint: impl AsRef<str>) -> RequestBuilder {
        self.client
            .request(method, format!("{}{}", BONUSLY_API_URL, endpoint.as_ref()))
            .bearer_auth(self.token.expose_secret())
            .header("HTTP_APPLICATION_NAME", "cherries-4-prs")
    }

    #[instrument(skip_all)]
    pub async fn list_users(&self) -> eyre::Result<Vec<User>> {
        const LIMIT: usize = 100;
        let mut skip: usize = 0;
        // Based roughly on Starry's size at time of writing.
        let mut ret = Vec::with_capacity(1000);

        loop {
            let mut users = self
                .request(reqwest::Method::GET, "/users")
                .query(&[("limit", LIMIT.to_string()), ("skip", skip.to_string())])
                .send()
                .await?
                .json::<BonuslyResult<Vec<User>>>()
                .await?
                .into_result()
                .map_err(Report::msg)?;
            let done = users.len() < LIMIT;
            skip += users.len();
            ret.append(&mut users);
            if done {
                break;
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        Ok(ret)
    }

    pub async fn my_email(&self) -> eyre::Result<String> {
        Ok(self.me().await?.email)
    }

    pub async fn me(&self) -> eyre::Result<User> {
        self.request(reqwest::Method::GET, "/users/me")
            .send()
            .await?
            .json::<BonuslyResult<User>>()
            .await?
            .into_result()
            .map_err(Report::msg)
    }

    pub async fn company(&self) -> eyre::Result<Company> {
        self.request(reqwest::Method::GET, "/companies/show")
            .send()
            .await?
            .json::<BonuslyResult<Company>>()
            .await?
            .into_result()
            .map_err(Report::msg)
    }

    pub async fn hashtags(&self) -> eyre::Result<Vec<String>> {
        Ok(self.company().await?.company_hashtags)
    }

    pub async fn send_bonus(&self, bonus: &Bonus) -> eyre::Result<BonusReply> {
        self.request(reqwest::Method::POST, "/bonuses")
            .json(bonus)
            .send()
            .await?
            .json::<BonuslyResult<BonusReply>>()
            .await?
            .into_result()
            .map_err(Report::msg)
    }
}

#[derive(Clone, Deserialize)]
pub struct BonuslyResult<T> {
    #[allow(dead_code)]
    success: bool,
    #[serde(flatten)]
    data: BonuslyResultInner<T>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum BonuslyResultInner<T> {
    Ok { result: T },
    Err { message: String },
}

impl<T> BonuslyResult<T> {
    pub fn into_result(self) -> Result<T, String> {
        match self.data {
            BonuslyResultInner::Ok { result, .. } => Ok(result),
            BonuslyResultInner::Err { message, .. } => Err(message),
        }
    }
}

/// A user on Bonusly.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub short_name: String,
    pub full_name: String,
    pub display_name: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub can_receive: bool,
}

/// A company on Bonusly. Contains the list of hashtags.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Company {
    company_hashtags: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Bonus {
    pub giver_email: String,
    pub receiver_email: String,
    pub amount: usize,
    /// Includes the leading `#`!
    pub hashtag: String,
    pub reason: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BonusReply {
    id: String,
    created_at: String,
    reason: String,
}
