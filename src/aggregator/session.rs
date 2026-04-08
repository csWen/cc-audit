use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::aggregator::cost::estimate_cost;
use crate::parser::discovery::discover_projects;
use crate::parser::jsonl::parse_jsonl;
use crate::parser::models::{ContentBlock, TranscriptEntry};

/// A single message in the rendered conversation.
pub struct ConversationMessage {
    pub role: String,       // "user" | "assistant"
    pub timestamp: String,
    pub model: String,
    pub blocks: Vec<DisplayBlock>,
}

/// A renderable content block.
pub enum DisplayBlock {
    /// Rendered HTML from markdown text.
    Text(String),
    /// Tool invocation: name, summary line, full input JSON.
    ToolUse {
        name: String,
        summary: String,
        input_json: String,
    },
    /// Tool result paired with its tool_use: tool name, content text, line count.
    ToolResult {
        tool_name: String,
        content: String,
        line_count: usize,
        truncated: bool,
    },
}

/// Metadata about a session for the detail page header.
pub struct SessionMeta {
    pub session_id: String,
    pub slug: String,
    pub project_display_name: String,
    pub project_dir_name: String,
    pub first_active: String,
    pub message_count: usize,
    pub total_tokens: String,
    pub cost: String,
}

/// Full session detail result.
pub struct SessionDetail {
    pub meta: SessionMeta,
    pub messages: Vec<ConversationMessage>,
}

const MAX_TOOL_RESULT_LINES: usize = 200;

/// Find the JSONL file for a given session_id across all projects.
fn find_session_file(claude_dir: &Path, session_id: &str) -> Option<(std::path::PathBuf, String, String)> {
    let projects = discover_projects(claude_dir).ok()?;
    let filename = format!("{session_id}.jsonl");

    for project in &projects {
        for jsonl_file in &project.jsonl_files {
            if let Some(name) = jsonl_file.file_name().and_then(|n| n.to_str()) {
                if name == filename {
                    return Some((
                        jsonl_file.clone(),
                        project.display_name.clone(),
                        project.dir_name.clone(),
                    ));
                }
            }
        }
    }
    None
}

/// Render markdown text to HTML using pulldown-cmark.
fn render_markdown(text: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(text, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Build a one-line summary for a tool_use block.
fn tool_use_summary(name: &str, input: &Option<serde_json::Value>) -> String {
    let Some(input) = input else {
        return name.to_string();
    };

    match name {
        "Read" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Read({path})")
        }
        "Write" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Write({path})")
        }
        "Edit" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Edit({path})")
        }
        "Grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = input.get("path").and_then(|v| v.as_str());
            match path {
                Some(p) => format!("Grep(pattern: \"{pattern}\", path: \"{p}\")"),
                None => format!("Grep(pattern: \"{pattern}\")"),
            }
        }
        "Glob" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Glob(pattern: \"{pattern}\")")
        }
        "Bash" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let truncated = if cmd.len() > 80 { &cmd[..80] } else { cmd };
            format!("Bash({truncated})")
        }
        "Agent" => {
            let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("?");
            let agent_type = input.get("subagent_type").and_then(|v| v.as_str());
            match agent_type {
                Some(t) => format!("Agent({t}: \"{desc}\")"),
                None => format!("Agent(\"{desc}\")"),
            }
        }
        "Skill" => {
            let skill = input.get("skill").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Skill({skill})")
        }
        _ => {
            // For MCP tools and others, show name + first string param
            if let Some(obj) = input.as_object() {
                let params: Vec<String> = obj
                    .iter()
                    .take(3)
                    .map(|(k, v)| {
                        let val = match v {
                            serde_json::Value::String(s) => {
                                if s.len() > 50 {
                                    format!("\"{}...\"", &s[..50])
                                } else {
                                    format!("\"{s}\"")
                                }
                            }
                            other => {
                                let s = other.to_string();
                                if s.len() > 50 {
                                    format!("{}...", &s[..50])
                                } else {
                                    s
                                }
                            }
                        };
                        format!("{k}: {val}")
                    })
                    .collect();
                format!("{name}({})", params.join(", "))
            } else {
                name.to_string()
            }
        }
    }
}

/// Extract text from a tool_result content (can be string or array of blocks).
fn extract_tool_result_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    parts.push(text.to_string());
                }
            }
            parts.join("\n")
        }
        _ => content.to_string(),
    }
}

/// Extract user text from a user message content (handles both string and array).
fn extract_user_content_text(content: &serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let mut texts = Vec::new();
            for item in arr {
                match item.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            texts.push(text.to_string());
                        }
                    }
                    Some("tool_result") => {
                        // tool_result blocks are handled separately
                    }
                    _ => {}
                }
            }
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        _ => None,
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

/// Load and build the full session conversation.
pub fn load_session(claude_dir: &Path, session_id: &str) -> Result<Option<SessionDetail>> {
    let Some((jsonl_path, project_display_name, project_dir_name)) =
        find_session_file(claude_dir, session_id)
    else {
        return Ok(None);
    };

    let parse_result = parse_jsonl(&jsonl_path)?;

    // Filter entries for this session, sorted by timestamp
    let mut entries: Vec<TranscriptEntry> = parse_result
        .entries
        .into_iter()
        .filter(|e| {
            e.common()
                .and_then(|c| c.session_id.as_deref())
                .is_some_and(|sid| sid == session_id)
        })
        .collect();

    entries.sort_by(|a, b| {
        let ta = a.common().and_then(|c| c.timestamp);
        let tb = b.common().and_then(|c| c.timestamp);
        ta.cmp(&tb)
    });

    // Collect metadata
    let mut slug = String::new();
    let mut first_active: Option<DateTime<Utc>> = None;
    let mut total_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;
    let mut message_count: usize = 0;

    // Build a map of tool_use_id -> tool_name for pairing results
    let mut tool_name_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // First pass: collect metadata and tool name map
    for entry in &entries {
        if let Some(common) = entry.common() {
            if slug.is_empty() {
                if let Some(s) = &common.slug {
                    slug = s.clone();
                }
            }
            if let Some(ts) = common.timestamp {
                if first_active.is_none_or(|prev| ts < prev) {
                    first_active = Some(ts);
                }
            }
        }

        if let TranscriptEntry::Assistant(a) = entry {
            if a.message.stop_reason.is_some() {
                if let Some(usage) = &a.message.usage {
                    let model = a.message.model.as_deref().unwrap_or("unknown");
                    total_tokens += usage.input_tokens + usage.output_tokens
                        + usage.cache_creation_input_tokens
                        + usage.cache_read_input_tokens;
                    total_cost += estimate_cost(
                        model,
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.cache_creation_input_tokens,
                        usage.cache_read_input_tokens,
                    );
                    message_count += 1;
                }
            }

            // Collect tool_use ids and names
            for block in &a.message.content {
                if let ContentBlock::ToolUse { id, name, .. } = block {
                    if let Some(id) = id {
                        tool_name_map.insert(id.clone(), name.clone());
                    }
                }
            }
        }
    }

    // Second pass: build conversation messages
    let mut messages: Vec<ConversationMessage> = Vec::new();

    // Track which assistant UUIDs we've seen (to deduplicate streaming chunks)
    // We group consecutive assistant entries and merge their content blocks
    let mut i = 0;
    while i < entries.len() {
        let entry = &entries[i];

        match entry {
            TranscriptEntry::User(user) => {
                let timestamp = entry
                    .common()
                    .and_then(|c| c.timestamp)
                    .map(|t| t.format("%H:%M").to_string())
                    .unwrap_or_default();

                let content = &user.message.content;
                let mut blocks = Vec::new();

                // Extract user text
                if let Some(text) = extract_user_content_text(content) {
                    if !text.is_empty() {
                        blocks.push(DisplayBlock::Text(render_markdown(&text)));
                    }
                }

                // Extract tool_result blocks
                if let serde_json::Value::Array(arr) = content {
                    for item in arr {
                        if item.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            let tool_use_id = item
                                .get("tool_use_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let tool_name = tool_name_map
                                .get(tool_use_id)
                                .cloned()
                                .unwrap_or_else(|| "unknown".to_string());

                            if let Some(result_content) = item.get("content") {
                                let text = extract_tool_result_text(result_content);
                                let lines: Vec<&str> = text.lines().collect();
                                let line_count = lines.len();
                                let truncated = line_count > MAX_TOOL_RESULT_LINES;
                                let display_text = if truncated {
                                    lines[..MAX_TOOL_RESULT_LINES].join("\n")
                                } else {
                                    text
                                };

                                blocks.push(DisplayBlock::ToolResult {
                                    tool_name,
                                    content: display_text,
                                    line_count,
                                    truncated,
                                });
                            }
                        }
                    }
                }

                // Only add user messages that have actual content (skip pure tool_result-only messages)
                let has_text = blocks.iter().any(|b| matches!(b, DisplayBlock::Text(_)));
                let has_tool_results = blocks
                    .iter()
                    .any(|b| matches!(b, DisplayBlock::ToolResult { .. }));

                if has_text {
                    // User message with text - include it (tool results go to a separate message)
                    let text_blocks: Vec<DisplayBlock> = blocks
                        .into_iter()
                        .filter(|b| matches!(b, DisplayBlock::Text(_)))
                        .collect();
                    messages.push(ConversationMessage {
                        role: "user".to_string(),
                        timestamp: timestamp.clone(),
                        model: String::new(),
                        blocks: text_blocks,
                    });
                } else if has_tool_results {
                    // Pure tool result message - attach results to the previous assistant message
                    if let Some(last_msg) = messages.last_mut() {
                        if last_msg.role == "assistant" {
                            for block in blocks {
                                if matches!(block, DisplayBlock::ToolResult { .. }) {
                                    last_msg.blocks.push(block);
                                }
                            }
                        }
                    }
                    i += 1;
                    continue;
                }

                i += 1;
            }

            TranscriptEntry::Assistant(a) => {
                let timestamp = entry
                    .common()
                    .and_then(|c| c.timestamp)
                    .map(|t| t.format("%H:%M").to_string())
                    .unwrap_or_default();
                let model = a
                    .message
                    .model
                    .as_deref()
                    .unwrap_or("")
                    .to_string();

                let mut blocks = Vec::new();

                // Collect all content blocks from this and consecutive assistant entries
                // (streaming chunks get merged)
                let mut j = i;
                let mut final_timestamp = timestamp.clone();
                let mut final_model = model.clone();

                while j < entries.len() {
                    if let TranscriptEntry::Assistant(asst) = &entries[j] {
                        // Update timestamp/model from latest chunk
                        if let Some(ts) = entries[j].common().and_then(|c| c.timestamp) {
                            final_timestamp = ts.format("%H:%M").to_string();
                        }
                        if let Some(m) = &asst.message.model {
                            final_model = m.clone();
                        }

                        for block in &asst.message.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    if !text.is_empty() {
                                        blocks.push(DisplayBlock::Text(render_markdown(text)));
                                    }
                                }
                                ContentBlock::ToolUse { name, input, .. } => {
                                    let summary = tool_use_summary(name, input);
                                    let input_json = input
                                        .as_ref()
                                        .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                                        .unwrap_or_default();
                                    blocks.push(DisplayBlock::ToolUse {
                                        name: name.clone(),
                                        summary,
                                        input_json,
                                    });
                                }
                                ContentBlock::Thinking { .. } => {
                                    // Skip thinking blocks
                                }
                                ContentBlock::ToolResult { .. } => {
                                    // Tool results in assistant messages are rare, skip
                                }
                            }
                        }

                        j += 1;

                        // If next entry is also assistant (streaming chunk), continue merging
                        if j < entries.len() {
                            if let TranscriptEntry::Assistant(_) = &entries[j] {
                                continue;
                            }
                        }
                        break;
                    } else {
                        break;
                    }
                }

                if !blocks.is_empty() {
                    messages.push(ConversationMessage {
                        role: "assistant".to_string(),
                        timestamp: final_timestamp,
                        model: final_model,
                        blocks,
                    });
                }

                i = j;
            }

            // Skip other entry types
            _ => {
                i += 1;
            }
        }
    }

    let meta = SessionMeta {
        session_id: session_id.to_string(),
        slug: if slug.is_empty() {
            if session_id.len() > 8 {
                format!("{}...", &session_id[..8])
            } else {
                session_id.to_string()
            }
        } else {
            slug
        },
        project_display_name,
        project_dir_name,
        first_active: first_active
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default(),
        message_count,
        total_tokens: fmt_tokens(total_tokens),
        cost: format!("{:.2}", total_cost),
    };

    Ok(Some(SessionDetail { meta, messages }))
}
