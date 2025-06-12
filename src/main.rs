use crate::api::{GetDiscussionOptionsBuilder, get_discussion};
use crate::config::Config;
use clap::{Parser, Subcommand};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod api;
mod config;
mod entity;

#[derive(Parser)]
struct Cli {
    #[arg(long, short)]
    config: Option<String>,
    #[command(subcommand)]
    cmd: SubCmd,
}
#[derive(Subcommand, Clone)]
enum SubCmd {
    Cron,
    Full,
}
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or("config.yml".to_string());
    let config = Config::load(config_path.as_str()).await.unwrap();
    let res = get_discussion(
        16677,
        GetDiscussionOptionsBuilder::default()
            .base_url(config.base_url.to_string())
            .concurrency(config.concurrency)
            .build()
            .unwrap(),
        None,
    )
    .await
    .unwrap();
    dbg!(res);
}
