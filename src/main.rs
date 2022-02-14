#![allow(unused_imports)]

use std::{collections::HashMap, convert::TryInto, fs::read_to_string};

use color_eyre::eyre::{self, WrapErr};
use secrecy::SecretString;
use serde::Deserialize;
use structopt::StructOpt;
use tracing::{event, info, instrument, span, warn, Level};

use cherries_4_prs::*;

#[instrument]
#[tokio::main]
pub async fn main() -> eyre::Result<()> {
    let args = Opt::from_args();
    install_tracing(&args.tracing_filter);
    color_eyre::install()?;

    let creds: Credentials = toml::de::from_str(&read_to_string("xxx_creds.toml")?)?;
    let cfg: Config = toml::de::from_str(&read_to_string("xxx_config.toml")?)?;
    let users: Vec<bonusly::User> =
        serde_json::from_str(&read_to_string("xxx_bonusly_users.json")?)?;

    let _bonusly = bonusly::Client::from_token(creds.bonusly);

    let github = octocrab::Octocrab::builder()
        .personal_token(creds.github)
        .build()?;

    let members: Vec<github::User> = github
        .get(format!("orgs/{}/members", cfg.github.org), None::<&()>)
        .await?;

    // xxx_reviews(cfg, github, users)?.await;

    // - correlate bonusly users <-> github users
    // - watch for changes over time

    Ok(())
}

async fn xxx_reviews(
    cfg: Config,
    github: octocrab::Octocrab,
    users: Vec<bonusly::User>,
) -> eyre::Result<()> {
    let updated_prs = cfg
        .github
        .prs_since(&github, "2021-08-20T00:00:00-04:00")
        .await?;
    for pr in updated_prs.items {
        let (org, repo) = github::org_repo(&pr)
            .ok_or_else(|| eyre::eyre!("Couldn't get org/repo from url {}", &pr.repository_url))?;

        let reviews = github
                .pulls(org, repo)
                .list_reviews(pr.number.try_into().expect("why did this api use different types for pr numbers and pr ids and then use the wrong one"))
                .await?;

        for review in reviews.items {
            if let Some(octocrab::models::pulls::ReviewState::Approved) = review.state {
                let user = github::User::from_login(&github, &review.user.login).await?;
                let email = cfg.find_bonusly_email(&users, &user);
                println!(
                    "pr {} to {}/{} approved by {}{}",
                    pr.number,
                    org,
                    repo,
                    review.user.login,
                    match email {
                        Some(email) => format!(" ({})", email),
                        None => "".to_owned(),
                    }
                );
            }
        }
    }
    Ok(())
}

fn install_tracing(filter_directives: &str) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{
        fmt::{self, format::FmtSpan, time::ChronoLocal},
        EnvFilter,
    };

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_span_events(FmtSpan::ACTIVE)
        .with_timer(ChronoLocal::rfc3339());
    let filter_layer = EnvFilter::try_new(filter_directives)
        .or_else(|_| EnvFilter::try_from_default_env())
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[derive(Debug, StructOpt)]
struct Opt {
    /// Tracing filter.
    ///
    /// Can be any of "error", "warn", "info", "debug", or "trace". Supports
    /// more granular filtering, as well.
    ///
    /// See: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html
    #[structopt(long, default_value = "info")]
    tracing_filter: String,
}
