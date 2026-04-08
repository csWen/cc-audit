use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::session_index::parse_session_index;

/// Represents a discovered Claude Code project directory.
#[derive(Debug, Clone)]
pub struct ProjectDir {
    /// The encoded directory name (e.g., "-Users-alice-project-my-app")
    pub dir_name: String,
    /// The restored original project path (e.g., "/Users/alice/project/my-app").
    /// Obtained from sessions-index.json when available, otherwise falls back to dir_name.
    pub project_path: String,
    /// A short display name (last two path components)
    pub display_name: String,
    /// Full path to the project directory under ~/.claude/projects/
    pub full_path: PathBuf,
    /// JSONL files found in this project directory
    pub jsonl_files: Vec<PathBuf>,
}

/// Scan ~/.claude/projects/ and discover all project directories.
pub fn discover_projects(claude_dir: &Path) -> Result<Vec<ProjectDir>> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();
    for entry in std::fs::read_dir(&projects_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Collect JSONL files
        let jsonl_files = collect_jsonl_files(&path);
        if jsonl_files.is_empty() {
            continue; // Skip projects with no transcript data
        }

        // Try to get projectPath from sessions-index.json, then from JSONL cwd field
        let project_path = resolve_project_path(&path, &dir_name, &jsonl_files);

        let display_name = make_display_name(&project_path);

        projects.push(ProjectDir {
            dir_name,
            project_path,
            display_name,
            full_path: path,
            jsonl_files,
        });
    }

    projects.sort_by(|a, b| a.project_path.cmp(&b.project_path));
    Ok(projects)
}

/// Try to resolve the original project path using multiple strategies:
/// 1. sessions-index.json's projectPath field
/// 2. "cwd" field from the first assistant message in a JSONL file
/// 3. Fall back to the encoded directory name
fn resolve_project_path(project_dir: &Path, dir_name: &str, jsonl_files: &[PathBuf]) -> String {
    // Strategy 1: sessions-index.json
    if let Ok(Some(index)) = parse_session_index(project_dir) {
        if let Some(path) = index.entries.first().and_then(|e| e.project_path.clone()) {
            return path;
        }
    }

    // Strategy 2: extract "cwd" from first JSONL file
    if let Some(first_jsonl) = jsonl_files.first() {
        if let Some(cwd) = extract_cwd_from_jsonl(first_jsonl) {
            return cwd;
        }
    }

    // Strategy 3: fall back to dir name
    dir_name.to_string()
}

/// Read the first few lines of a JSONL file to find a "cwd" field.
fn extract_cwd_from_jsonl(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(20) {
        let line = line.ok()?;
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(cwd) = val.get("cwd").and_then(|v| v.as_str()) {
                return Some(cwd.to_string());
            }
        }
    }
    None
}

/// Collect all top-level JSONL files in a project directory.
fn collect_jsonl_files(project_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(project_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Make a short display name from a project path (last two components).
fn make_display_name(project_path: &str) -> String {
    let parts: Vec<&str> = project_path.trim_end_matches('/').rsplit('/').collect();
    match parts.len() {
        0 => project_path.to_string(),
        1 => parts[0].to_string(),
        _ => format!("{}/{}", parts[1], parts[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_name() {
        assert_eq!(
            make_display_name("/Users/alice/project/my-app"),
            "project/my-app"
        );
        assert_eq!(
            make_display_name("/Users/alice/work/org/service"),
            "org/service"
        );
    }

    #[test]
    fn test_discover_real_projects() {
        let home = std::env::var("HOME").unwrap();
        let claude_dir = PathBuf::from(&home).join(".claude");
        if !claude_dir.is_dir() {
            return; // Skip if no Claude data
        }

        let projects = discover_projects(&claude_dir).unwrap();
        assert!(!projects.is_empty(), "Should discover at least one project");

        for p in &projects {
            println!(
                "  {} ({}) - {} jsonl files",
                p.display_name,
                p.project_path,
                p.jsonl_files.len()
            );
        }
    }
}
