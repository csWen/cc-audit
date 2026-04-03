use std::collections::HashMap;
use std::path::PathBuf;

use cc_audit::parser::discovery::discover_projects;
use cc_audit::parser::jsonl::parse_jsonl;
use cc_audit::parser::models::TranscriptEntry;

fn claude_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(&home).join(".claude");
    dir.is_dir().then_some(dir)
}

#[test]
fn test_parse_all_lines_100_percent() {
    let claude_dir = match claude_dir() {
        Some(d) => d,
        None => return,
    };

    let projects = discover_projects(&claude_dir).unwrap();
    println!("Found {} projects with JSONL files", projects.len());

    let mut total_lines = 0usize;
    let mut total_success = 0usize;
    let mut total_failed = 0usize;
    let mut type_counts: HashMap<String, usize> = HashMap::new();

    for project in &projects {
        for jsonl_file in &project.jsonl_files {
            let result = parse_jsonl(jsonl_file).unwrap();
            total_lines += result.total_lines;
            total_success += result.success_lines;
            total_failed += result.failed_lines;

            for entry in &result.entries {
                *type_counts
                    .entry(entry.type_name().to_string())
                    .or_default() += 1;
            }
        }
    }

    println!("\n=== Parse Statistics ===");
    println!("Total lines:   {total_lines}");
    println!("Success:       {total_success}");
    println!("Failed:        {total_failed}");

    let success_rate = if total_lines > 0 {
        total_success as f64 / total_lines as f64 * 100.0
    } else {
        0.0
    };
    println!("Success rate:  {success_rate:.1}%");

    println!("\n=== Type Distribution ===");
    let mut types: Vec<_> = type_counts.into_iter().collect();
    types.sort_by(|a, b| b.1.cmp(&a.1));
    for (type_name, count) in &types {
        println!("  {type_name:<25} {count:>6}");
    }

    assert!(
        success_rate >= 95.0,
        "Parse success rate {success_rate:.1}% is below 95% target"
    );
}

#[test]
fn test_extract_usage_and_tools() {
    let claude_dir = match claude_dir() {
        Some(d) => d,
        None => return,
    };

    let projects = discover_projects(&claude_dir).unwrap();

    let mut total_tokens = 0u64;
    let mut tool_counts: HashMap<String, usize> = HashMap::new();
    let mut model_counts: HashMap<String, usize> = HashMap::new();
    let mut assistant_count = 0usize;
    let mut with_usage_count = 0usize;

    for project in &projects {
        for jsonl_file in &project.jsonl_files {
            let result = parse_jsonl(jsonl_file).unwrap();
            for entry in &result.entries {
                if let TranscriptEntry::Assistant(a) = entry {
                    assistant_count += 1;

                    // Usage extraction
                    if let Some(usage) = &a.message.usage {
                        with_usage_count += 1;
                        total_tokens += usage.input_tokens
                            + usage.output_tokens
                            + usage.cache_creation_input_tokens
                            + usage.cache_read_input_tokens;
                    }

                    // Model extraction
                    if let Some(model) = &a.message.model {
                        *model_counts.entry(model.clone()).or_default() += 1;
                    }

                    // Tool use extraction
                    for block in &a.message.content {
                        if let cc_audit::parser::models::ContentBlock::ToolUse {
                            name, ..
                        } = block
                        {
                            *tool_counts.entry(name.clone()).or_default() += 1;
                        }
                    }
                }
            }
        }
    }

    println!("\n=== Usage & Tools ===");
    println!("Assistant messages: {assistant_count}");
    println!("With usage data:   {with_usage_count}");
    println!("Total tokens:      {total_tokens}");

    println!("\n=== Models ===");
    let mut models: Vec<_> = model_counts.into_iter().collect();
    models.sort_by(|a, b| b.1.cmp(&a.1));
    for (model, count) in &models {
        println!("  {model:<30} {count:>6}");
    }

    println!("\n=== Top Tools ===");
    let mut tools: Vec<_> = tool_counts.into_iter().collect();
    tools.sort_by(|a, b| b.1.cmp(&a.1));
    for (tool, count) in tools.iter().take(15) {
        println!("  {tool:<30} {count:>6}");
    }

    assert!(assistant_count > 0, "Should have assistant messages");
    assert!(with_usage_count > 0, "Should have usage data");
    assert!(total_tokens > 0, "Should have token counts");
    assert!(!tools.is_empty(), "Should have tool calls");
}
