use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};

use crate::parser::discovery::{discover_projects, ProjectDir};
use crate::parser::jsonl::parse_jsonl;
use crate::parser::models::{ContentBlock, TranscriptEntry};

use super::cost::estimate_cost;

// ── Time range filter ──

#[derive(Debug, Clone, Copy)]
pub enum TimeRange {
    Today,
    Days7,
    Days30,
    All,
}

impl TimeRange {
    pub fn label(&self) -> &str {
        match self {
            TimeRange::Today => "today",
            TimeRange::Days7 => "last 7 days",
            TimeRange::Days30 => "last 30 days",
            TimeRange::All => "all time",
        }
    }

    pub fn cutoff(&self) -> Option<DateTime<Utc>> {
        let now = Utc::now();
        match self {
            TimeRange::Today => {
                let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
                Some(today_start.and_utc())
            }
            TimeRange::Days7 => Some(now - chrono::Duration::days(7)),
            TimeRange::Days30 => Some(now - chrono::Duration::days(30)),
            TimeRange::All => None,
        }
    }
}

// ── Aggregation result structs ──

#[derive(Debug, Default, Clone)]
pub struct TokenBreakdown {
    pub input: u64,
    pub output: u64,
    pub cache_create: u64,
    pub cache_read: u64,
}

impl TokenBreakdown {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_create + self.cache_read
    }
}

#[derive(Debug, Default)]
pub struct ModelUsage {
    pub name: String,
    pub tokens: TokenBreakdown,
    pub message_count: usize,
    pub cost: f64,
}

#[derive(Debug, Default)]
pub struct ProjectStats {
    pub dir_name: String,
    pub project_path: String,
    pub display_name: String,
    pub tokens: TokenBreakdown,
    pub cost: f64,
    pub session_count: usize,
    pub message_count: usize,
}

#[derive(Debug, Default)]
pub struct ToolCallStats {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Default)]
pub struct SkillCallStats {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Default)]
pub struct AgentCallStats {
    pub agent_type: String,
    pub count: usize,
}

#[derive(Debug, Default)]
pub struct DailyTokenUsage {
    pub date: NaiveDate,
    pub tokens: TokenBreakdown,
    pub cost: f64,
}

#[derive(Debug, Default)]
pub struct GlobalStats {
    pub time_range: String,
    pub total_sessions: usize,
    pub total_turns: usize,
    pub tokens: TokenBreakdown,
    pub total_cost: f64,
    pub projects: Vec<ProjectStats>,
    pub models: Vec<ModelUsage>,
    pub tools: Vec<ToolCallStats>,
    pub skills: Vec<SkillCallStats>,
    pub agents: Vec<AgentCallStats>,
    pub daily: Vec<DailyTokenUsage>,
}

// ── Aggregation logic ──

pub fn aggregate(claude_dir: &Path, time_range: TimeRange) -> Result<GlobalStats> {
    let projects = discover_projects(claude_dir)?;
    let cutoff = time_range.cutoff();

    let mut stats = GlobalStats {
        time_range: time_range.label().to_string(),
        ..Default::default()
    };

    let mut model_map: HashMap<String, (TokenBreakdown, usize, f64)> = HashMap::new();
    let mut tool_map: HashMap<String, usize> = HashMap::new();
    let mut skill_map: HashMap<String, usize> = HashMap::new();
    let mut agent_map: HashMap<String, usize> = HashMap::new();
    let mut daily_map: HashMap<NaiveDate, (TokenBreakdown, f64)> = HashMap::new();

    for project in &projects {
        let mut proj_stats = ProjectStats {
            dir_name: project.dir_name.clone(),
            project_path: project.project_path.clone(),
            display_name: project.display_name.clone(),
            ..Default::default()
        };

        let mut sessions_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for jsonl_file in &project.jsonl_files {
            let parse_result = parse_jsonl(jsonl_file)?;

            for entry in &parse_result.entries {
                // Time filter
                if let Some(cutoff) = cutoff {
                    if let Some(common) = entry.common() {
                        if let Some(ts) = common.timestamp {
                            if ts < cutoff {
                                continue;
                            }
                        }
                    }
                }

                // Track sessions
                if let Some(common) = entry.common() {
                    if let Some(sid) = &common.session_id {
                        sessions_seen.insert(sid.clone());
                    }
                }

                if let TranscriptEntry::Assistant(a) = entry {
                    proj_stats.message_count += 1;
                    stats.total_turns += 1;

                    let model = a
                        .message
                        .model
                        .as_deref()
                        .unwrap_or("unknown");

                    // Usage aggregation
                    if let Some(usage) = &a.message.usage {
                        let cost = estimate_cost(
                            model,
                            usage.input_tokens,
                            usage.output_tokens,
                            usage.cache_creation_input_tokens,
                            usage.cache_read_input_tokens,
                        );

                        // Project tokens
                        proj_stats.tokens.input += usage.input_tokens;
                        proj_stats.tokens.output += usage.output_tokens;
                        proj_stats.tokens.cache_create += usage.cache_creation_input_tokens;
                        proj_stats.tokens.cache_read += usage.cache_read_input_tokens;
                        proj_stats.cost += cost;

                        // Global tokens
                        stats.tokens.input += usage.input_tokens;
                        stats.tokens.output += usage.output_tokens;
                        stats.tokens.cache_create += usage.cache_creation_input_tokens;
                        stats.tokens.cache_read += usage.cache_read_input_tokens;
                        stats.total_cost += cost;

                        // Model breakdown
                        let m = model_map.entry(model.to_string()).or_default();
                        m.0.input += usage.input_tokens;
                        m.0.output += usage.output_tokens;
                        m.0.cache_create += usage.cache_creation_input_tokens;
                        m.0.cache_read += usage.cache_read_input_tokens;
                        m.1 += 1;
                        m.2 += cost;

                        // Daily breakdown
                        if let Some(common) = entry.common() {
                            if let Some(ts) = common.timestamp {
                                let date = ts.date_naive();
                                let d = daily_map.entry(date).or_default();
                                d.0.input += usage.input_tokens;
                                d.0.output += usage.output_tokens;
                                d.0.cache_create += usage.cache_creation_input_tokens;
                                d.0.cache_read += usage.cache_read_input_tokens;
                                d.1 += cost;
                            }
                        }
                    }

                    // Tool/Skill/Agent extraction
                    for block in &a.message.content {
                        if let ContentBlock::ToolUse { name, input, .. } = block {
                            *tool_map.entry(name.clone()).or_default() += 1;

                            if name == "Skill" {
                                if let Some(input) = input {
                                    if let Some(skill_name) = input.get("skill").and_then(|v| v.as_str()) {
                                        *skill_map.entry(skill_name.to_string()).or_default() += 1;
                                    }
                                }
                            }

                            if name == "Agent" {
                                if let Some(input) = input {
                                    let agent_type = input
                                        .get("subagent_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("general-purpose");
                                    *agent_map.entry(agent_type.to_string()).or_default() += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        proj_stats.session_count = sessions_seen.len();
        stats.total_sessions += proj_stats.session_count;

        if proj_stats.tokens.total() > 0 {
            stats.projects.push(proj_stats);
        }
    }

    // Sort and convert maps to vecs
    stats
        .projects
        .sort_by(|a, b| b.tokens.total().cmp(&a.tokens.total()));

    stats.models = model_map
        .into_iter()
        .map(|(name, (tokens, message_count, cost))| ModelUsage {
            name,
            tokens,
            message_count,
            cost,
        })
        .collect();
    stats.models.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap());

    stats.tools = tool_map
        .into_iter()
        .map(|(name, count)| ToolCallStats { name, count })
        .collect();
    stats.tools.sort_by(|a, b| b.count.cmp(&a.count));

    stats.skills = skill_map
        .into_iter()
        .map(|(name, count)| SkillCallStats { name, count })
        .collect();
    stats.skills.sort_by(|a, b| b.count.cmp(&a.count));

    stats.agents = agent_map
        .into_iter()
        .map(|(agent_type, count)| AgentCallStats { agent_type, count })
        .collect();
    stats.agents.sort_by(|a, b| b.count.cmp(&a.count));

    stats.daily = daily_map
        .into_iter()
        .map(|(date, (tokens, cost))| DailyTokenUsage { date, tokens, cost })
        .collect();
    stats.daily.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(stats)
}
