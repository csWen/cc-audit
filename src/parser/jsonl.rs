use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::Result;

use super::models::TranscriptEntry;

/// Result of parsing a single JSONL file.
#[derive(Debug, Default)]
pub struct ParseResult {
    pub entries: Vec<TranscriptEntry>,
    pub total_lines: usize,
    pub success_lines: usize,
    pub failed_lines: usize,
}

/// Parse a single JSONL transcript file, skipping lines that fail to parse.
pub fn parse_jsonl(path: &Path) -> Result<ParseResult> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut result = ParseResult::default();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        result.total_lines += 1;

        match TranscriptEntry::parse(trimmed) {
            Ok(entry) => {
                result.success_lines += 1;
                result.entries.push(entry);
            }
            Err(e) => {
                result.failed_lines += 1;
                eprintln!(
                    "Warning: failed to parse line {} in {}: {}",
                    result.total_lines,
                    path.display(),
                    e
                );
            }
        }
    }

    Ok(result)
}

/// Parse all JSONL files in a project directory (excluding subagent files).
pub fn parse_project_jsonl_files(project_dir: &Path) -> Result<ParseResult> {
    let mut combined = ParseResult::default();

    for entry in std::fs::read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only parse top-level .jsonl files (not subagent files in subdirs)
        if path.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            let result = parse_jsonl(&path)?;
            combined.total_lines += result.total_lines;
            combined.success_lines += result.success_lines;
            combined.failed_lines += result.failed_lines;
            combined.entries.extend(result.entries);
        }
    }

    Ok(combined)
}
