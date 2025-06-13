use crate::cmd::Cmd;
use crate::config::Config;
use crate::db::get_connection_pool;
use clap::{Parser, Subcommand};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod api;
mod cmd;
mod config;
mod crawler;
mod db;
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
    Retry,
    Full {
        #[arg(long)]
        min: u64,
        #[arg(long)]
        max: u64,
        #[arg(short, long)]
        ignore_existed: bool,
    },
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
    let conn = get_connection_pool(config.db.as_str()).await.unwrap();
    let cmd = Cmd::new(config, conn);
    match cli.cmd {
        SubCmd::Cron => {}
        SubCmd::Full {
            min,
            max,
            ignore_existed,
        } => {
            cmd.full(min, max, ignore_existed).await;
        }
        SubCmd::Retry => {
            cmd.retry().await;
        }
    }
}
