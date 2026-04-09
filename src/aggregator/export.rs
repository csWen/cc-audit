use crate::aggregator::session::{DisplayBlock, SessionDetail};

/// Render session conversation as Markdown.
pub fn render_markdown(detail: &SessionDetail, include_tools: bool) -> String {
    let meta = &detail.meta;
    let mut out = String::new();

    // Header
    out.push_str(&format!("# Session: {}\n\n", meta.slug));
    out.push_str(&format!(
        "- **Project**: {}\n- **Date**: {}\n- **Messages**: {} | **Tokens**: {} | **Cost**: ${}\n\n",
        meta.project_display_name, meta.first_active, meta.message_count, meta.total_tokens, meta.cost
    ));
    out.push_str("---\n\n");

    for msg in &detail.messages {
        // Role heading
        if msg.role == "user" {
            out.push_str(&format!("### User — {}\n\n", msg.timestamp));
        } else {
            let model_part = if msg.model.is_empty() {
                String::new()
            } else {
                format!(" ({})", msg.model)
            };
            out.push_str(&format!(
                "### Assistant — {}{}\n\n",
                msg.timestamp, model_part
            ));
        }

        for block in &msg.blocks {
            match block {
                DisplayBlock::Text(html) => {
                    let text = html_to_plain_text(html);
                    if !text.is_empty() {
                        out.push_str(&text);
                        out.push_str("\n\n");
                    }
                }
                DisplayBlock::ToolUse {
                    name,
                    summary,
                    input_json,
                } => {
                    if include_tools {
                        out.push_str(&format!(
                            "<details>\n<summary><b>{name}</b> — {summary}</summary>\n\n"
                        ));
                        if !input_json.is_empty() {
                            out.push_str("```json\n");
                            out.push_str(input_json);
                            out.push_str("\n```\n");
                        }
                        out.push_str("</details>\n\n");
                    }
                }
                DisplayBlock::ToolResult {
                    tool_name,
                    content,
                    line_count,
                    truncated,
                } => {
                    if include_tools {
                        let label = if *truncated {
                            format!("{tool_name} result ({line_count} lines, truncated)")
                        } else {
                            format!("{tool_name} result ({line_count} lines)")
                        };
                        out.push_str(&format!("<details>\n<summary>{label}</summary>\n\n```\n"));
                        out.push_str(content);
                        out.push_str("\n```\n</details>\n\n");
                    }
                }
            }
        }

        out.push_str("---\n\n");
    }

    out
}

/// Render session conversation as a self-contained HTML document.
pub fn render_html(detail: &SessionDetail, include_tools: bool) -> String {
    let meta = &detail.meta;
    let mut out = String::new();

    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    out.push_str(&format!(
        "<title>Session: {} — {}</title>\n",
        html_escape(&meta.slug),
        html_escape(&meta.project_display_name)
    ));
    out.push_str("<style>\n");
    out.push_str(EXPORT_CSS);
    out.push_str("\n</style>\n</head>\n<body>\n");

    // Header
    out.push_str("<div class=\"header\">\n");
    out.push_str(&format!("<h1>{}</h1>\n", html_escape(&meta.slug)));
    out.push_str(&format!(
        "<div class=\"meta\">Project: {} &middot; {} &middot; {} messages &middot; {} tokens &middot; ${}</div>\n",
        html_escape(&meta.project_display_name),
        html_escape(&meta.first_active),
        meta.message_count,
        html_escape(&meta.total_tokens),
        html_escape(&meta.cost),
    ));
    out.push_str("</div>\n\n");

    // Messages
    for msg in &detail.messages {
        let role_class = if msg.role == "user" {
            "user"
        } else {
            "assistant"
        };
        out.push_str(&format!("<div class=\"msg msg-{role_class}\">\n"));

        // Header
        out.push_str("<div class=\"msg-header\">");
        if msg.role == "user" {
            out.push_str("<span class=\"badge badge-user\">User</span>");
        } else {
            out.push_str("<span class=\"badge badge-assistant\">Assistant</span>");
            if !msg.model.is_empty() {
                out.push_str(&format!(
                    " <span class=\"model\">{}</span>",
                    html_escape(&msg.model)
                ));
            }
        }
        out.push_str(&format!(
            " <span class=\"time\">{}</span>",
            html_escape(&msg.timestamp)
        ));
        out.push_str("</div>\n");

        // Body
        out.push_str("<div class=\"msg-body\">\n");
        for block in &msg.blocks {
            match block {
                DisplayBlock::Text(html) => {
                    out.push_str("<div class=\"text\">");
                    out.push_str(html);
                    out.push_str("</div>\n");
                }
                DisplayBlock::ToolUse {
                    name,
                    summary,
                    input_json,
                } => {
                    if include_tools {
                        out.push_str("<div class=\"tool-call\">");
                        out.push_str(&format!(
                            "<div class=\"tool-header\"><span class=\"dot\"></span><span class=\"tool-name\">{}</span> <span class=\"tool-summary\">{}</span></div>",
                            html_escape(name),
                            html_escape(summary),
                        ));
                        if !input_json.is_empty() {
                            out.push_str(
                                "<details class=\"tool-details\"><summary>Input</summary><pre>",
                            );
                            out.push_str(&html_escape(input_json));
                            out.push_str("</pre></details>");
                        }
                        out.push_str("</div>\n");
                    }
                }
                DisplayBlock::ToolResult {
                    tool_name: _,
                    content,
                    line_count,
                    truncated,
                } => {
                    if include_tools {
                        let label = if *truncated {
                            format!("Result ({line_count} lines, truncated)")
                        } else {
                            format!("Result ({line_count} lines)")
                        };
                        out.push_str(&format!(
                            "<div class=\"tool-result\"><details class=\"tool-details\"><summary>{label}</summary><pre>"
                        ));
                        out.push_str(&html_escape(content));
                        out.push_str("</pre></details></div>\n");
                    }
                }
            }
        }
        out.push_str("</div>\n</div>\n\n");
    }

    out.push_str("</body>\n</html>\n");
    out
}

/// Strip HTML tags and decode common entities to recover plain text.
///
/// The input is pre-rendered markdown HTML from pulldown-cmark.
/// We preserve structural whitespace (newlines for block elements, etc.)
/// so the output reads naturally in a markdown file.
fn html_to_plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_pre = false;
    let mut tag_buf = String::new();

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag_lower = tag_buf.to_lowercase();
                let tag_name = tag_lower.split_whitespace().next().unwrap_or("");

                match tag_name {
                    "pre" => in_pre = true,
                    "/pre" => in_pre = false,
                    "br" | "br/" => out.push('\n'),
                    "/p" | "/div" | "/h1" | "/h2" | "/h3" | "/h4" | "/h5" | "/h6" | "/li"
                    | "/tr" | "/blockquote" => {
                        out.push('\n');
                    }
                    _ => {}
                }
            } else {
                tag_buf.push(ch);
            }
            continue;
        }

        // Decode entities inline
        out.push(ch);
    }

    // Decode common HTML entities
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ");

    // Clean up excessive blank lines
    let mut result = String::new();
    let mut prev_blank = false;
    for line in out.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push('\n');
                prev_blank = true;
            }
        } else {
            if in_pre {
                result.push_str(line);
            } else {
                result.push_str(trimmed);
            }
            result.push('\n');
            prev_blank = false;
        }
    }

    result.trim().to_string()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const EXPORT_CSS: &str = r#"
body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    max-width: 900px;
    margin: 0 auto;
    padding: 24px;
    background: #f8fafc;
    color: #0f172a;
}
.header { margin-bottom: 24px; border-bottom: 2px solid #e2e8f0; padding-bottom: 16px; }
.header h1 { font-size: 22px; margin: 0 0 6px 0; }
.meta { font-size: 13px; color: #64748b; }
.msg { background: #fff; border-radius: 8px; padding: 16px 20px; margin-bottom: 6px; box-shadow: 0 1px 3px rgba(0,0,0,0.06); }
.msg-header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
.badge { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; padding: 2px 8px; border-radius: 4px; }
.badge-user { color: #1d4ed8; background: #dbeafe; }
.badge-assistant { color: #059669; background: #d1fae5; }
.model { font-size: 11px; color: #64748b; font-family: monospace; }
.time { font-size: 11px; color: #94a3b8; margin-left: auto; }
.text { font-size: 14px; line-height: 1.65; }
.text p { margin: 0 0 8px 0; }
.text p:last-child { margin-bottom: 0; }
.text pre { background: #1e293b; color: #e2e8f0; padding: 12px 16px; border-radius: 6px; overflow-x: auto; font-size: 13px; line-height: 1.5; margin: 8px 0; }
.text code { background: #f1f5f9; padding: 1px 5px; border-radius: 3px; font-size: 13px; }
.text pre code { background: none; padding: 0; }
.text ul, .text ol { padding-left: 24px; margin: 8px 0; }
.text li { margin: 4px 0; }
.text table { border-collapse: collapse; margin: 8px 0; font-size: 13px; }
.text th, .text td { border: 1px solid #e2e8f0; padding: 6px 10px; text-align: left; }
.text th { background: #f8fafc; font-weight: 600; }
.text blockquote { border-left: 3px solid #3b82f6; margin: 8px 0; padding: 4px 16px; color: #64748b; }
.text h1, .text h2, .text h3, .text h4 { margin: 12px 0 6px 0; }
.tool-call, .tool-result { margin: 6px 0; font-size: 13px; }
.tool-header { display: flex; align-items: center; gap: 6px; padding: 4px 0; font-family: monospace; color: #64748b; }
.dot { width: 8px; height: 8px; border-radius: 50%; background: #3b82f6; display: inline-block; flex-shrink: 0; }
.tool-name { font-weight: 600; color: #3b82f6; }
.tool-summary { color: #64748b; }
.tool-details { margin: 4px 0 4px 14px; border-left: 2px solid #e2e8f0; padding-left: 12px; }
.tool-details summary { cursor: pointer; color: #64748b; font-size: 12px; padding: 2px 0; }
.tool-details pre { background: #1e293b; color: #e2e8f0; padding: 10px 14px; border-radius: 6px; overflow-x: auto; font-size: 12px; line-height: 1.4; margin: 6px 0; max-height: 400px; overflow-y: auto; }
"#;
