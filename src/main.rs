#![allow(unused_imports)]

use std::{collections::HashMap, convert::TryInto, fs::read_to_string, path::PathBuf};

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

    let prg = Program::from_config_path(&args.config).await?;

    prg.xxx_reviews().await?;

    Ok(())
}

fn install_tracing(filter_directives: &str) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{
        fmt::{self, format::FmtSpan, time::UtcTime},
        EnvFilter,
    };

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_span_events(FmtSpan::ACTIVE)
        .without_time();
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
    /// Configuration path (TOML).
    config: PathBuf,
}
