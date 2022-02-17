use std::path::PathBuf;

use color_eyre::eyre;
use structopt::StructOpt;
use tracing::info;

use cherries_4_prs::*;

#[tokio::main]
pub async fn main() -> eyre::Result<()> {
    let args = Opt::from_args();
    install_tracing(&args.tracing_filter);
    color_eyre::install()?;

    let mut prg = Program::from_config_path(args.config.clone()).await?;
    info!("Ready!");

    loop {
        prg.reply_all_and_wait().await?;
    }
}

fn install_tracing(filter_directives: &str) {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{
        fmt::{self, format::FmtSpan},
        EnvFilter,
    };

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .without_time();
    let filter_layer = EnvFilter::try_new(filter_directives)
        .or_else(|_| EnvFilter::try_from_default_env())
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter_layer)
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
