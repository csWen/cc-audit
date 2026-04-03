use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cc-audit", about = "Claude Code usage insight tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show usage statistics summary
    Stats,
    /// Start local web server for detailed dashboard
    Serve,
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Stats => {
            println!("cc-audit stats: coming soon...");
            Ok(())
        }
        Command::Serve => {
            println!("cc-audit serve: coming soon...");
            Ok(())
        }
    }
}
