use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::models::ConversationSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexThread {
    pub id: String,
    pub rollout_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: String,
    pub model_provider: String,
    pub cwd: String,
    pub title: String,
    pub preview: String,
    pub tokens_used: i64,
    pub archived: bool,
    pub git_sha: Option<String>,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub first_user_message: String,
}

impl CodexThread {
    pub fn to_conversation_summary(&self, codex_dir: &Path) -> ConversationSummary {
        let dt = chrono::DateTime::from_timestamp(self.updated_at, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| self.updated_at.to_string());

        let raw_title = if !self.title.is_empty() {
            &self.title
        } else if !self.first_user_message.is_empty() {
            &self.first_user_message
        } else {
            "Codex Session"
        };

        let first_line = raw_title.lines().next().unwrap_or("Codex Session").trim();
        let cleaned = first_line.trim_start_matches('#').trim();
        let title = if cleaned.is_empty() {
            let prefix = if self.id.len() >= 8 { &self.id[..8] } else { &self.id };
            format!("Codex Thread {}", prefix)
        } else {
            cleaned.to_string()
        };

        let preview = if !self.preview.is_empty() {
            self.preview.clone()
        } else {
            self.first_user_message.chars().take(120).collect()
        };

        let ws_uri = format!("[\"file://{}\"]", self.cwd);
        let agent_name = self.model.clone().unwrap_or_else(|| "codex".to_string());

        ConversationSummary {
            conversation_id: self.id.clone(),
            title,
            preview,
            step_count: 1,
            last_modified_time: dt.clone(),
            workspace_uris: ws_uri,
            status: if self.archived {
                "ARCHIVED".to_string()
            } else {
                "IDLE".to_string()
            },
            source: "codex".to_string(),
            project_id: self.cwd.clone(),
            agent_name,
            parent_conversation_id: String::new(),
            nesting_depth: 0,
            battle_id: String::new(),
            winning_conversation_id: String::new(),
            not_fully_idle: false,
            killed: false,
            last_user_input_time: dt,
            last_user_input_step_index: 0,
            app_data_dir: codex_dir.to_string_lossy().to_string(),
            raw_summary: None,
            group_id: String::new(),
        }
    }
}

pub struct CodexDb {
    db_path: PathBuf,
}

impl CodexDb {
    pub fn new(db_path: &Path) -> Self {
        Self {
            db_path: db_path.to_path_buf(),
        }
    }

    fn open_read_only(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| {
                format!("Failed to open Codex database at {:?} (read-only)", self.db_path)
            })?;
        let _ = conn.execute_batch("PRAGMA busy_timeout = 5000;");
        Ok(conn)
    }

    pub fn list_threads(&self, workspace_filter: Option<&str>) -> Result<Vec<CodexThread>> {
        let conn = self.open_read_only()?;
        let query = "SELECT 
            id, rollout_path, created_at, updated_at, source, model_provider, 
            cwd, title, preview, tokens_used, archived, git_sha, git_branch, 
            model, reasoning_effort, first_user_message
        FROM threads 
        ORDER BY updated_at DESC";

        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map([], |row| {
            let archived_int: i64 = row.get(10).unwrap_or(0);
            Ok(CodexThread {
                id: row.get(0)?,
                rollout_path: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                source: row.get(4)?,
                model_provider: row.get(5)?,
                cwd: row.get(6)?,
                title: row.get(7)?,
                preview: row.get(8)?,
                tokens_used: row.get(9).unwrap_or(0),
                archived: archived_int != 0,
                git_sha: row.get(11).ok(),
                git_branch: row.get(12).ok(),
                model: row.get(13).ok(),
                reasoning_effort: row.get(14).ok(),
                first_user_message: row.get(15).unwrap_or_default(),
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            let thread = r?;
            if let Some(ws) = workspace_filter {
                if !thread.cwd.to_lowercase().contains(&ws.to_lowercase()) {
                    continue;
                }
            }
            results.push(thread);
        }

        Ok(results)
    }

    pub fn get_thread(&self, id: &str) -> Result<Option<CodexThread>> {
        let conn = self.open_read_only()?;
        let query = "SELECT 
            id, rollout_path, created_at, updated_at, source, model_provider, 
            cwd, title, preview, tokens_used, archived, git_sha, git_branch, 
            model, reasoning_effort, first_user_message
        FROM threads 
        WHERE id = ?1 OR id LIKE ?1 || '%' 
        ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END 
        LIMIT 1";

        let mut stmt = conn.prepare(query)?;
        let mut rows = stmt.query_map([id], |row| {
            let archived_int: i64 = row.get(10).unwrap_or(0);
            Ok(CodexThread {
                id: row.get(0)?,
                rollout_path: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                source: row.get(4)?,
                model_provider: row.get(5)?,
                cwd: row.get(6)?,
                title: row.get(7)?,
                preview: row.get(8)?,
                tokens_used: row.get(9).unwrap_or(0),
                archived: archived_int != 0,
                git_sha: row.get(11).ok(),
                git_branch: row.get(12).ok(),
                model: row.get(13).ok(),
                reasoning_effort: row.get(14).ok(),
                first_user_message: row.get(15).unwrap_or_default(),
            })
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }
}
