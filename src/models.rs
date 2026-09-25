use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub title: String,
    pub preview: String,
    pub step_count: i64,
    pub last_modified_time: String,
    pub workspace_uris: String, // Stored as JSON string like ["file:///path/to/project"]
    pub status: String,
    pub source: String,
    pub project_id: String,
    pub agent_name: String,
    pub parent_conversation_id: String,
    pub nesting_depth: i64,
    pub battle_id: String,
    pub winning_conversation_id: String,
    pub not_fully_idle: bool,
    pub killed: bool,
    pub last_user_input_time: String,
    pub last_user_input_step_index: i64,
    pub app_data_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_summary: Option<Vec<u8>>,
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub version: String,
    pub created_at: String,
    pub source_os: String,
    pub source_username: String,
    pub conversation: ConversationSummary,
    pub sanitized: bool,
    pub compact: bool,
    pub original_workspace_paths: Vec<String>,
    pub artifacts_count: usize,
    pub ignored_count: usize,
    pub has_db: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExportOptions {
    pub conversation_id: String,
    pub output_path: PathBuf,
    pub sanitize: bool,
    pub compact: bool,
    pub workspace_root: Option<PathBuf>,
    pub custom_ignore_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub bundle_path: PathBuf,
    pub target_workspace: Option<PathBuf>,
    pub overwrite: bool,
    pub dry_run: bool,
}
