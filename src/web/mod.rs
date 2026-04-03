use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use askama::Template;
use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use chrono::NaiveDate;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};

use crate::aggregator::stats::{self, GlobalStats, ProjectDetailStats, TimeRange};

#[derive(Embed)]
#[folder = "static/"]
struct StaticAssets;

// ── Templates ──

#[derive(Template)]
#[template(path = "overview.html")]
struct OverviewPage {
    active_nav: &'static str,
    range: String,
}

#[derive(Template)]
#[template(path = "overview_partial.html")]
struct OverviewPartial {
    range: String,
    time_range_label: String,
    total_sessions: usize,
    total_turns: usize,
    total_tokens: String,
    input_tokens: String,
    output_tokens: String,
    cache_create_tokens: String,
    cache_read_tokens: String,
    total_cost: String,
    // Chart data as JSON
    daily_labels_json: String,
    daily_input_json: String,
    daily_output_json: String,
    daily_cache_create_json: String,
    daily_cache_read_json: String,
    model_labels_json: String,
    model_costs_json: String,
}

// ── Project list templates ──

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsPage {
    active_nav: &'static str,
    range: String,
}

struct ProjectRow {
    dir_name: String,
    display_name: String,
    project_path: String,
    session_count: usize,
    total_tokens: String,
    cost: String,
    last_active: String,
}

#[derive(Template)]
#[template(path = "projects_partial.html")]
struct ProjectsPartial {
    range: String,
    time_range_label: String,
    projects: Vec<ProjectRow>,
}

// ── Project detail templates ──

#[derive(Template)]
#[template(path = "project_detail.html")]
struct ProjectDetailPage {
    active_nav: &'static str,
    dir_name: String,
    display_name: String,
    range: String,
}

struct SessionRow {
    time: String,
    slug: String,
    message_count: usize,
    total_tokens: String,
    cost: String,
    first_prompt: String,
}

#[derive(Template)]
#[template(path = "project_detail_partial.html")]
struct ProjectDetailPartial {
    dir_name: String,
    display_name: String,
    range: String,
    time_range_label: String,
    total_tokens: String,
    total_cost: String,
    session_count: usize,
    sessions: Vec<SessionRow>,
    // Chart data
    daily_labels_json: String,
    daily_input_json: String,
    daily_output_json: String,
    daily_cache_create_json: String,
    daily_cache_read_json: String,
    tool_labels_json: String,
    tool_counts_json: String,
}

// ── Tools page templates ──

#[derive(Template)]
#[template(path = "tools.html")]
struct ToolsPage {
    active_nav: &'static str,
    range: String,
}

struct ToolRow {
    name: String,
    count: usize,
}

struct SkillRow {
    name: String,
    count: usize,
    last_used: String,
}

struct AgentRow {
    agent_type: String,
    count: usize,
    last_used: String,
}

#[derive(Serialize)]
struct TrendDataset {
    label: String,
    data: Vec<usize>,
}

#[derive(Template)]
#[template(path = "tools_partial.html")]
struct ToolsPartial {
    range: String,
    time_range_label: String,
    tools: Vec<ToolRow>,
    skills: Vec<SkillRow>,
    agents: Vec<AgentRow>,
    // Chart data
    tool_labels_json: String,
    tool_counts_json: String,
    trend_labels_json: String,
    trend_datasets_json: String,
}

// ── Query params ──

#[derive(Deserialize)]
struct RangeQuery {
    #[serde(default = "default_range")]
    range: String,
}

fn default_range() -> String {
    "7d".to_string()
}

fn parse_time_range(s: &str) -> TimeRange {
    match s {
        "today" => TimeRange::Today,
        "30d" => TimeRange::Days30,
        "all" => TimeRange::All,
        _ => TimeRange::Days7,
    }
}

// ── Shared state ──

struct AppState {
    claude_dir: PathBuf,
}

// ── Handlers ──

async fn overview_page(Query(q): Query<RangeQuery>) -> impl IntoResponse {
    let tmpl = OverviewPage {
        active_nav: "overview",
        range: q.range,
    };
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

async fn overview_partial(
    Query(q): Query<RangeQuery>,
    state: axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    let time_range = parse_time_range(&q.range);
    let stats = stats::aggregate(&state.claude_dir, time_range).unwrap_or_default();
    let tmpl = build_overview_partial(&q.range, &stats);
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

fn build_overview_partial(range: &str, stats: &GlobalStats) -> OverviewPartial {
    // Daily chart data
    let daily_labels: Vec<String> = stats
        .daily
        .iter()
        .map(|d| d.date.format("%m/%d").to_string())
        .collect();
    let daily_input: Vec<u64> = stats.daily.iter().map(|d| d.tokens.input).collect();
    let daily_output: Vec<u64> = stats.daily.iter().map(|d| d.tokens.output).collect();
    let daily_cache_create: Vec<u64> = stats.daily.iter().map(|d| d.tokens.cache_create).collect();
    let daily_cache_read: Vec<u64> = stats.daily.iter().map(|d| d.tokens.cache_read).collect();

    // Model chart data
    let model_labels: Vec<String> = stats.models.iter().map(|m| m.name.clone()).collect();
    let model_costs: Vec<f64> = stats.models.iter().map(|m| m.cost).collect();

    OverviewPartial {
        range: range.to_string(),
        time_range_label: stats.time_range.clone(),
        total_sessions: stats.total_sessions,
        total_turns: stats.total_turns,
        total_tokens: fmt_tokens(stats.tokens.total()),
        input_tokens: fmt_tokens(stats.tokens.input),
        output_tokens: fmt_tokens(stats.tokens.output),
        cache_create_tokens: fmt_tokens(stats.tokens.cache_create),
        cache_read_tokens: fmt_tokens(stats.tokens.cache_read),
        total_cost: format!("{:.2}", stats.total_cost),
        daily_labels_json: serde_json::to_string(&daily_labels).unwrap_or_default(),
        daily_input_json: serde_json::to_string(&daily_input).unwrap_or_default(),
        daily_output_json: serde_json::to_string(&daily_output).unwrap_or_default(),
        daily_cache_create_json: serde_json::to_string(&daily_cache_create).unwrap_or_default(),
        daily_cache_read_json: serde_json::to_string(&daily_cache_read).unwrap_or_default(),
        model_labels_json: serde_json::to_string(&model_labels).unwrap_or_default(),
        model_costs_json: serde_json::to_string(&model_costs).unwrap_or_default(),
    }
}

// ── Project list handlers ──

async fn projects_page(Query(q): Query<RangeQuery>) -> impl IntoResponse {
    let tmpl = ProjectsPage {
        active_nav: "projects",
        range: q.range,
    };
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

async fn projects_partial(
    Query(q): Query<RangeQuery>,
    state: axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    let time_range = parse_time_range(&q.range);
    let global_stats = stats::aggregate(&state.claude_dir, time_range).unwrap_or_default();

    let projects: Vec<ProjectRow> = global_stats
        .projects
        .iter()
        .map(|p| ProjectRow {
            dir_name: p.dir_name.clone(),
            display_name: p.display_name.clone(),
            project_path: p.project_path.clone(),
            session_count: p.session_count,
            total_tokens: fmt_tokens(p.tokens.total()),
            cost: format!("{:.2}", p.cost),
            last_active: p
                .last_active
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
        })
        .collect();

    let tmpl = ProjectsPartial {
        range: q.range,
        time_range_label: global_stats.time_range,
        projects,
    };
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

// ── Project detail handlers ──

#[derive(Deserialize)]
struct ProjectDetailPath {
    dir_name: String,
}

async fn project_detail_page(
    axum::extract::Path(path): axum::extract::Path<ProjectDetailPath>,
    Query(q): Query<RangeQuery>,
    state: axum::extract::State<Arc<AppState>>,
) -> Response {
    // We need the display_name, so do a quick lookup
    let display_name = stats::aggregate_project(&state.claude_dir, &path.dir_name, TimeRange::All)
        .ok()
        .flatten()
        .map(|d| d.display_name)
        .unwrap_or_else(|| path.dir_name.clone());

    let tmpl = ProjectDetailPage {
        active_nav: "projects",
        dir_name: path.dir_name,
        display_name,
        range: q.range,
    };
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}"))).into_response()
}

async fn project_detail_partial(
    axum::extract::Path(path): axum::extract::Path<ProjectDetailPath>,
    Query(q): Query<RangeQuery>,
    state: axum::extract::State<Arc<AppState>>,
) -> Response {
    let time_range = parse_time_range(&q.range);
    let detail = stats::aggregate_project(&state.claude_dir, &path.dir_name, time_range)
        .ok()
        .flatten();

    let Some(detail) = detail else {
        return Html("<div style='text-align:center; padding:60px; color:#64748b;'>Project not found.</div>".to_string()).into_response();
    };

    let tmpl = build_project_detail_partial(&q.range, &detail);
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}"))).into_response()
}

fn build_project_detail_partial(range: &str, detail: &ProjectDetailStats) -> ProjectDetailPartial {
    let daily_labels: Vec<String> = detail.daily.iter().map(|d| d.date.format("%m/%d").to_string()).collect();
    let daily_input: Vec<u64> = detail.daily.iter().map(|d| d.tokens.input).collect();
    let daily_output: Vec<u64> = detail.daily.iter().map(|d| d.tokens.output).collect();
    let daily_cache_create: Vec<u64> = detail.daily.iter().map(|d| d.tokens.cache_create).collect();
    let daily_cache_read: Vec<u64> = detail.daily.iter().map(|d| d.tokens.cache_read).collect();

    let tool_labels: Vec<String> = detail.tools.iter().take(15).map(|t| t.name.clone()).collect();
    let tool_counts: Vec<usize> = detail.tools.iter().take(15).map(|t| t.count).collect();

    let sessions: Vec<SessionRow> = detail
        .sessions
        .iter()
        .map(|s| SessionRow {
            time: s
                .first_active
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
            slug: s.slug.clone(),
            message_count: s.message_count,
            total_tokens: fmt_tokens(s.tokens.total()),
            cost: format!("{:.2}", s.cost),
            first_prompt: s.first_prompt.clone(),
        })
        .collect();

    ProjectDetailPartial {
        dir_name: detail.dir_name.clone(),
        display_name: detail.display_name.clone(),
        range: range.to_string(),
        time_range_label: detail.time_range.clone(),
        total_tokens: fmt_tokens(detail.tokens.total()),
        total_cost: format!("{:.2}", detail.cost),
        session_count: detail.session_count,
        sessions,
        daily_labels_json: serde_json::to_string(&daily_labels).unwrap_or_default(),
        daily_input_json: serde_json::to_string(&daily_input).unwrap_or_default(),
        daily_output_json: serde_json::to_string(&daily_output).unwrap_or_default(),
        daily_cache_create_json: serde_json::to_string(&daily_cache_create).unwrap_or_default(),
        daily_cache_read_json: serde_json::to_string(&daily_cache_read).unwrap_or_default(),
        tool_labels_json: serde_json::to_string(&tool_labels).unwrap_or_default(),
        tool_counts_json: serde_json::to_string(&tool_counts).unwrap_or_default(),
    }
}

// ── Tools page handlers ──

async fn tools_page(Query(q): Query<RangeQuery>) -> impl IntoResponse {
    let tmpl = ToolsPage {
        active_nav: "tools",
        range: q.range,
    };
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

async fn tools_partial(
    Query(q): Query<RangeQuery>,
    state: axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    let time_range = parse_time_range(&q.range);
    let global_stats = stats::aggregate(&state.claude_dir, time_range).unwrap_or_default();
    let tmpl = build_tools_partial(&q.range, &global_stats);
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {e}")))
}

fn build_tools_partial(range: &str, stats: &GlobalStats) -> ToolsPartial {
    let tools: Vec<ToolRow> = stats
        .tools
        .iter()
        .map(|t| ToolRow {
            name: t.name.clone(),
            count: t.count,
        })
        .collect();

    let skills: Vec<SkillRow> = stats
        .skills
        .iter()
        .map(|s| SkillRow {
            name: s.name.clone(),
            count: s.count,
            last_used: s
                .last_used
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
        })
        .collect();

    let agents: Vec<AgentRow> = stats
        .agents
        .iter()
        .map(|a| AgentRow {
            agent_type: a.agent_type.clone(),
            count: a.count,
            last_used: a
                .last_used
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
        })
        .collect();

    // Bar chart: top 15 tools
    let top_tools: Vec<&stats::ToolCallStats> = stats.tools.iter().take(15).collect();
    let tool_labels: Vec<String> = top_tools.iter().map(|t| t.name.clone()).collect();
    let tool_counts: Vec<usize> = top_tools.iter().map(|t| t.count).collect();

    // Trend chart: top 5 tools daily trend
    let top5_names: Vec<String> = stats.tools.iter().take(5).map(|t| t.name.clone()).collect();
    let trend_labels: Vec<String> = stats
        .daily_tool_calls
        .iter()
        .map(|d| d.date.format("%m/%d").to_string())
        .collect();

    let trend_datasets: Vec<TrendDataset> = top5_names
        .iter()
        .map(|name| {
            let data: Vec<usize> = stats
                .daily_tool_calls
                .iter()
                .map(|d| {
                    d.counts
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, c)| *c)
                        .unwrap_or(0)
                })
                .collect();
            TrendDataset {
                label: name.clone(),
                data,
            }
        })
        .collect();

    ToolsPartial {
        range: range.to_string(),
        time_range_label: stats.time_range.clone(),
        tools,
        skills,
        agents,
        tool_labels_json: serde_json::to_string(&tool_labels).unwrap_or_default(),
        tool_counts_json: serde_json::to_string(&tool_counts).unwrap_or_default(),
        trend_labels_json: serde_json::to_string(&trend_labels).unwrap_or_default(),
        trend_datasets_json: serde_json::to_string(&trend_datasets).unwrap_or_default(),
    }
}

// ── Static file handler ──

async fn static_file(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(file) => {
            let mime = if path.ends_with(".js") {
                "application/javascript"
            } else if path.ends_with(".css") {
                "text/css"
            } else {
                "application/octet-stream"
            };
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime)],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
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

// ── Server entry point ──

pub async fn serve(port: u16) -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let claude_dir = PathBuf::from(&home).join(".claude");

    if !claude_dir.is_dir() {
        anyhow::bail!("~/.claude directory not found");
    }

    let state = Arc::new(AppState { claude_dir });

    let app = Router::new()
        .route("/", get(overview_page))
        .route("/api/overview", get(overview_partial))
        .route("/projects", get(projects_page))
        .route("/api/projects", get(projects_partial))
        .route("/projects/{dir_name}", get(project_detail_page))
        .route("/api/project-detail/{dir_name}", get(project_detail_partial))
        .route("/tools", get(tools_page))
        .route("/api/tools", get(tools_partial))
        .route("/static/{*path}", get(static_file))
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    println!("CC-Audit server running at http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
