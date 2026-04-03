use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SessionIndex {
    pub version: Option<u64>,
    pub entries: Vec<SessionEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub session_id: String,
    pub full_path: Option<String>,
    pub file_mtime: Option<u64>,
    pub first_prompt: Option<String>,
    pub summary: Option<String>,
    pub message_count: Option<u64>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub project_path: Option<String>,
    pub is_sidechain: Option<bool>,
}

/// Parse a sessions-index.json file if it exists.
pub fn parse_session_index(project_dir: &Path) -> Result<Option<SessionIndex>> {
    let index_path = project_dir.join("sessions-index.json");
    if !index_path.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&index_path)?;
    let index: SessionIndex = serde_json::from_str(&content)?;
    Ok(Some(index))
}
