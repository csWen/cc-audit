mod aggregator;
mod cli;
mod parser;
mod web;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli::run(cli).await
}
