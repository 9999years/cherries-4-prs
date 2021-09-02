#![allow(unused_imports)]

use std::{convert::TryInto, fs::read_to_string};

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

    let bonusly = Bonusly::from_token(creds.bonusly);

    // println!("{:#?}", bonusly.list_users().await?);

    let github = octocrab::Octocrab::builder()
        .personal_token(creds.github)
        .build()?;

    let updated_prs = github
        .search()
        .issues_and_pull_requests(&format!(
            "is:pr author:{} review:approved org:{} updated:>=2021-08-20T00:00:00-04:00",
            cfg.github.user, cfg.github.org
        ))
        .send()
        .await?;

    for pr in updated_prs.items {
        let (org, repo) = {
            let mut segments = pr
                .repository_url
                .path_segments()
                .ok_or_else(|| eyre::eyre!("bad repo path"))?;
            segments.next();
            (
                segments.next().ok_or_else(|| eyre::eyre!("no org"))?,
                segments.next().ok_or_else(|| eyre::eyre!("no repo"))?,
            )
        };
        // println!("org: {}, repo: {}, pr: {}", org, repo, pr.id.0);
        let reviews = github
            .pulls(org, repo)
            .list_reviews(pr.number.try_into().expect("why did this api use different types for pr numbers and pr ids and then use the wrong one"))
            .await?;

        for review in reviews.items {
            if let Some(octocrab::models::pulls::ReviewState::Approved) = review.state {
                println!(
                    "pr {} to {}/{} approved by {}",
                    pr.number, org, repo, review.user.login
                );
            }
        }
    }

    // next: get reviews
    // https://docs.rs/octocrab/0.12.0/octocrab/pulls/struct.PullRequestHandler.html#method.list_reviews

    // - correlate bonusly users <-> github users
    // - watch for changes over time

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

#[derive(Deserialize, Clone)]
struct Credentials {
    bonusly: String,
    github: String,
}

#[derive(Deserialize, Clone)]
struct Config {
    github: GitHubConfig,
}

#[derive(Deserialize, Clone)]
struct GitHubConfig {
    user: String,
    org: String,
}
