use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use clap::{Parser, Subcommand, ValueEnum};

use crate::aggregator::stats::{self, GlobalStats, SessionSummary, TimeRange};
use crate::parser::discovery::find_project_for_path;

#[derive(Parser)]
#[command(name = "cc-audit", about = "Claude Code usage insight tool", version)]
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
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// List recent sessions of a project, newest first (handy for `claude --resume <id>`)
    Sessions {
        /// Project directory; any subdirectory of the project also works
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Maximum number of sessions to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
        /// How much detail to print per session
        #[arg(short, long, default_value = "brief")]
        format: SessionFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum SessionFormat {
    /// Session ids only, one per line (pipe-friendly)
    Id,
    /// One line per session: id, last active time, title
    Brief,
    /// Multi-line block per session with activity window, usage, and first prompt
    Full,
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

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Stats { range } => run_stats(range.into()),
        Command::Serve { port } => crate::web::serve(port).await,
        Command::Sessions {
            path,
            limit,
            format,
        } => run_sessions(&path, limit, format),
    }
}

fn claude_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")?;
    let claude_dir = PathBuf::from(&home).join(".claude");
    if !claude_dir.is_dir() {
        anyhow::bail!("~/.claude directory not found");
    }
    Ok(claude_dir)
}

fn run_stats(time_range: TimeRange) -> anyhow::Result<()> {
    let stats = stats::aggregate(&claude_dir()?, time_range)?;
    print_stats(&stats);
    Ok(())
}

fn run_sessions(path: &Path, limit: usize, format: SessionFormat) -> anyhow::Result<()> {
    let Some(project) = find_project_for_path(&claude_dir()?, path)? else {
        anyhow::bail!(
            "no Claude Code project found for {}",
            path.canonicalize()
                .unwrap_or_else(|_| path.to_path_buf())
                .display()
        );
    };

    let mut sessions = stats::aggregate_project_dir(&project, TimeRange::All)?.sessions;
    sessions.sort_by(|a, b| b.last_active.cmp(&a.last_active));
    sessions.truncate(limit);

    if !matches!(format, SessionFormat::Id) {
        println!(
            "Sessions for {} ({}) — {} most recent",
            project.display_name,
            project.project_path,
            sessions.len()
        );
        println!();
    }

    for sess in &sessions {
        match format {
            SessionFormat::Id => println!("{}", sess.session_id),
            SessionFormat::Brief => println!(
                "{}  {}  {}",
                sess.session_id,
                fmt_local_time(sess.last_active),
                sess.display_title()
            ),
            SessionFormat::Full => print_session_full(sess),
        }
    }
    Ok(())
}

fn print_session_full(sess: &SessionSummary) {
    println!("{}", sess.session_id);
    println!("  Title:     {}", sess.display_title());
    println!(
        "  Active:    {} → {}",
        fmt_local_time(sess.first_active),
        fmt_local_time(sess.last_active)
    );
    println!(
        "  Usage:     {} messages, {} tokens, ${:.2}",
        sess.message_count,
        fmt_tokens(sess.tokens.total()),
        sess.cost
    );
    if !sess.first_prompt.is_empty() {
        println!("  Prompt:    {}", sess.first_prompt.replace('\n', " "));
    }
    println!("  Resume:    claude --resume {}", sess.session_id);
    println!();
}

fn fmt_local_time(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn print_stats(stats: &GlobalStats) {
    println!("CC-Audit Stats ({})", stats.time_range);
    println!("─────────────────────────────────────────");

    println!("Total sessions:  {}", stats.total_sessions);
    println!("Total turns:     {}", stats.total_turns);
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
            println!("  {:<25} {:>5.1}%   ${:.2}", m.name, pct, m.cost);
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
