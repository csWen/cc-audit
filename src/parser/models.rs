use chrono::{DateTime, Utc};
use serde::Deserialize;

/// A single line entry from a transcript JSONL file.
///
/// Known types are parsed into specific variants. Unknown types are captured
/// as `Other` with the raw type string, ensuring forward compatibility.
#[derive(Debug)]
pub enum TranscriptEntry {
    Assistant(AssistantEntry),
    User(UserEntry),
    System(SystemEntry),
    PermissionMode(PermissionModeEntry),
    FileHistorySnapshot(FileHistorySnapshotEntry),
    Progress(ProgressEntry),
    LastPrompt(LastPromptEntry),
    PrLink(PrLinkEntry),
    /// Catch-all for unknown/future message types
    Other { type_name: String },
}

/// Internal serde-tagged enum for known types only.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum KnownEntry {
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(rename = "user")]
    User(UserEntry),
    #[serde(rename = "system")]
    System(SystemEntry),
    #[serde(rename = "permission-mode")]
    PermissionMode(PermissionModeEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(FileHistorySnapshotEntry),
    #[serde(rename = "progress")]
    Progress(ProgressEntry),
    #[serde(rename = "last-prompt")]
    LastPrompt(LastPromptEntry),
    #[serde(rename = "pr-link")]
    PrLink(PrLinkEntry),
}

impl TranscriptEntry {
    /// Parse a JSON string into a TranscriptEntry.
    /// Known types are strongly typed; unknown types become `Other`.
    pub fn parse(json_str: &str) -> Result<Self, serde_json::Error> {
        match serde_json::from_str::<KnownEntry>(json_str) {
            Ok(known) => Ok(known.into()),
            Err(_) => {
                // Try to extract just the "type" field
                #[derive(Deserialize)]
                struct TypeOnly {
                    r#type: String,
                }
                match serde_json::from_str::<TypeOnly>(json_str) {
                    Ok(t) => Ok(TranscriptEntry::Other {
                        type_name: t.r#type,
                    }),
                    Err(e) => Err(e),
                }
            }
        }
    }
}

impl From<KnownEntry> for TranscriptEntry {
    fn from(entry: KnownEntry) -> Self {
        match entry {
            KnownEntry::Assistant(e) => TranscriptEntry::Assistant(e),
            KnownEntry::User(e) => TranscriptEntry::User(e),
            KnownEntry::System(e) => TranscriptEntry::System(e),
            KnownEntry::PermissionMode(e) => TranscriptEntry::PermissionMode(e),
            KnownEntry::FileHistorySnapshot(e) => TranscriptEntry::FileHistorySnapshot(e),
            KnownEntry::Progress(e) => TranscriptEntry::Progress(e),
            KnownEntry::LastPrompt(e) => TranscriptEntry::LastPrompt(e),
            KnownEntry::PrLink(e) => TranscriptEntry::PrLink(e),
        }
    }
}

// ── Common fields ──

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct CommonFields {
    pub uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "isSidechain")]
    pub is_sidechain: Option<bool>,
    #[serde(rename = "userType")]
    pub user_type: Option<String>,
    pub entrypoint: Option<String>,
    pub cwd: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    pub slug: Option<String>,
}

// ── Assistant ──

#[derive(Debug, Deserialize)]
pub struct AssistantEntry {
    pub message: AssistantMessage,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    pub model: Option<String>,
    pub id: Option<String>,
    pub role: Option<String>,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: Option<String>,
        signature: Option<String>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: Option<String>,
        name: String,
        input: Option<serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: Option<String>,
        content: Option<serde_json::Value>,
    },
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

// ── User ──

#[derive(Debug, Deserialize)]
pub struct UserEntry {
    pub message: UserMessage,
    #[serde(rename = "promptId")]
    pub prompt_id: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Deserialize)]
pub struct UserMessage {
    pub role: Option<String>,
    pub content: serde_json::Value,
}

// ── System ──

#[derive(Debug, Deserialize)]
pub struct SystemEntry {
    pub subtype: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "messageCount")]
    pub message_count: Option<u64>,
    #[serde(flatten)]
    pub common: CommonFields,
}

// ── Permission Mode ──

#[derive(Debug, Deserialize)]
pub struct PermissionModeEntry {
    #[serde(rename = "permissionMode")]
    pub permission_mode: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

// ── File History Snapshot ──

#[derive(Debug, Deserialize)]
pub struct FileHistorySnapshotEntry {
    #[serde(rename = "messageId")]
    pub message_id: Option<String>,
    pub snapshot: Option<serde_json::Value>,
    #[serde(rename = "isSnapshotUpdate")]
    pub is_snapshot_update: Option<bool>,
    #[serde(flatten)]
    pub common: CommonFields,
}

// ── Progress ──

#[derive(Debug, Deserialize)]
pub struct ProgressEntry {
    #[serde(flatten)]
    pub common: CommonFields,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ── PR Link ──

#[derive(Debug, Deserialize)]
pub struct PrLinkEntry {
    #[serde(rename = "prNumber")]
    pub pr_number: Option<u64>,
    #[serde(rename = "prUrl")]
    pub pr_url: Option<String>,
    #[serde(rename = "prRepository")]
    pub pr_repository: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

// ── Last Prompt ──

#[derive(Debug, Deserialize)]
pub struct LastPromptEntry {
    #[serde(rename = "lastPrompt")]
    pub last_prompt: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

impl TranscriptEntry {
    pub fn common(&self) -> Option<&CommonFields> {
        match self {
            TranscriptEntry::Assistant(e) => Some(&e.common),
            TranscriptEntry::User(e) => Some(&e.common),
            TranscriptEntry::System(e) => Some(&e.common),
            TranscriptEntry::PermissionMode(e) => Some(&e.common),
            TranscriptEntry::FileHistorySnapshot(e) => Some(&e.common),
            TranscriptEntry::Progress(e) => Some(&e.common),
            TranscriptEntry::LastPrompt(e) => Some(&e.common),
            TranscriptEntry::PrLink(e) => Some(&e.common),
            TranscriptEntry::Other { .. } => None,
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            TranscriptEntry::Assistant(_) => "assistant",
            TranscriptEntry::User(_) => "user",
            TranscriptEntry::System(_) => "system",
            TranscriptEntry::PermissionMode(_) => "permission-mode",
            TranscriptEntry::FileHistorySnapshot(_) => "file-history-snapshot",
            TranscriptEntry::Progress(_) => "progress",
            TranscriptEntry::LastPrompt(_) => "last-prompt",
            TranscriptEntry::PrLink(_) => "pr-link",
            TranscriptEntry::Other { type_name } => type_name,
        }
    }
}
