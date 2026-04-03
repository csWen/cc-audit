use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::aggregator::stats::{self, GlobalStats, TimeRange, TokenBreakdown};

#[derive(Parser)]
#[command(name = "cc-audit", about = "Claude Code usage insight tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show usage statistics summary
    Stats {
        /// Time range filter
        #[arg(short, long, default_value = "7d")]
        range: RangeArg,
    },
    /// Start local web server for detailed dashboard
    Serve,
}

#[derive(Clone, ValueEnum)]
pub enum RangeArg {
    Today,
    #[value(name = "7d")]
    Days7,
    #[value(name = "30d")]
    Days30,
    All,
}

impl From<RangeArg> for TimeRange {
    fn from(arg: RangeArg) -> Self {
        match arg {
            RangeArg::Today => TimeRange::Today,
            RangeArg::Days7 => TimeRange::Days7,
            RangeArg::Days30 => TimeRange::Days30,
            RangeArg::All => TimeRange::All,
        }
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Stats { range } => run_stats(range.into()),
        Command::Serve => {
            println!("cc-audit serve: coming soon...");
            Ok(())
        }
    }
}

fn run_stats(time_range: TimeRange) -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let claude_dir = PathBuf::from(&home).join(".claude");

    if !claude_dir.is_dir() {
        anyhow::bail!("~/.claude directory not found");
    }

    let stats = stats::aggregate(&claude_dir, time_range)?;
    print_stats(&stats);
    Ok(())
}

fn print_stats(stats: &GlobalStats) {
    println!("CC-Audit Stats ({})", stats.time_range);
    println!("─────────────────────────────────────────");

    println!(
        "Total sessions:  {}",
        stats.total_sessions
    );
    println!(
        "Total turns:     {}",
        stats.total_turns
    );
    println!(
        "Total tokens:    {} (input: {}, output: {}, cache_create: {}, cache_read: {})",
        fmt_tokens(stats.tokens.total()),
        fmt_tokens(stats.tokens.input),
        fmt_tokens(stats.tokens.output),
        fmt_tokens(stats.tokens.cache_create),
        fmt_tokens(stats.tokens.cache_read),
    );
    println!("Total cost:      ${:.2} (estimated)", stats.total_cost);

    // Top projects
    if !stats.projects.is_empty() {
        println!("\nTop projects:");
        for (i, p) in stats.projects.iter().take(10).enumerate() {
            println!(
                "  {:>2}. {:<35} {:>10}   ${:.2}",
                i + 1,
                p.display_name,
                fmt_tokens(p.tokens.total()),
                p.cost,
            );
        }
    }

    // Top tools
    if !stats.tools.is_empty() {
        println!("\nTop tools:");
        for (i, t) in stats.tools.iter().take(10).enumerate() {
            println!("  {:>2}. {:<25} {:>6} calls", i + 1, t.name, t.count);
        }
    }

    // Skills
    if !stats.skills.is_empty() {
        println!("\nSkills:");
        for s in &stats.skills {
            println!("  {:<35} {:>4} calls", s.name, s.count);
        }
    }

    // Agents
    if !stats.agents.is_empty() {
        println!("\nAgents:");
        for a in &stats.agents {
            println!("  {:<25} {:>4} calls", a.agent_type, a.count);
        }
    }

    // Models
    if !stats.models.is_empty() {
        println!("\nModels:");
        let total_cost = stats.total_cost.max(0.01);
        for m in &stats.models {
            let pct = m.cost / total_cost * 100.0;
            println!(
                "  {:<25} {:>5.1}%   ${:.2}",
                m.name, pct, m.cost
            );
        }
    }
}

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
