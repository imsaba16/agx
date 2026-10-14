use crate::models::ConversationSummary;
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;

pub struct Database {
    db_path: std::path::PathBuf,
}

impl Database {
    pub fn new(db_path: &Path) -> Self {
        Self {
            db_path: db_path.to_path_buf(),
        }
    }

    fn open_read_only(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| {
                format!("Failed to open database at {:?} (read-only)", self.db_path)
            })?;
        let _ = conn.execute_batch("PRAGMA busy_timeout = 5000;");
        Ok(conn)
    }

    fn open_read_write(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            &self.db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .with_context(|| format!("Failed to open database at {:?}", self.db_path))?;
        let _ = conn.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL;");
        Ok(conn)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let conn = self.open_read_only()?;
        let mut stmt = conn.prepare(
            "SELECT 
                conversation_id, title, preview, step_count, last_modified_time, 
                workspace_uris, status, source, project_id, agent_name, 
                parent_conversation_id, nesting_depth, battle_id, winning_conversation_id, 
                not_fully_idle, killed, last_user_input_time, last_user_input_step_index, 
                app_data_dir, raw_summary, group_id
            FROM conversation_summaries 
            ORDER BY datetime(last_modified_time) DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ConversationSummary {
                conversation_id: row.get(0)?,
                title: row.get(1)?,
                preview: row.get(2)?,
                step_count: row.get(3)?,
                last_modified_time: row.get(4)?,
                workspace_uris: row.get(5)?,
                status: row.get(6)?,
                source: row.get(7)?,
                project_id: row.get(8)?,
                agent_name: row.get(9)?,
                parent_conversation_id: row.get(10)?,
                nesting_depth: row.get(11)?,
                battle_id: row.get(12)?,
                winning_conversation_id: row.get(13)?,
                not_fully_idle: row.get::<_, i64>(14)? != 0,
                killed: row.get::<_, i64>(15)? != 0,
                last_user_input_time: row.get(16)?,
                last_user_input_step_index: row.get(17)?,
                app_data_dir: row.get(18)?,
                raw_summary: row.get(19)?,
                group_id: row.get(20)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn find_conversations_for_workspace(
        &self,
        workspace_path: &Path,
    ) -> Result<Vec<ConversationSummary>> {
        let raw_path = workspace_path.to_path_buf();
        let cleaned = crate::remapper::clean_path(workspace_path);

        let mut terms = vec![
            raw_path.to_string_lossy().to_string(),
            cleaned.to_string_lossy().to_string(),
            crate::remapper::to_file_uri(&raw_path),
            crate::remapper::to_file_uri(&cleaned),
        ];

        // On macOS, canonicalization prepends /private to /var/folders or /tmp
        if let Some(stripped) = cleaned.to_str().and_then(|s| s.strip_prefix("/private")) {
            terms.push(stripped.to_string());
            terms.push(format!("file://{}", stripped));
        }

        let mut all_terms = Vec::new();
        for t in &terms {
            all_terms.push(t.clone());
            all_terms.push(t.replace('\\', "/"));
            all_terms.push(t.to_lowercase());
            all_terms.push(t.replace('\\', "/").to_lowercase());
            all_terms.push(t.replace(':', "%3a").to_lowercase());
            all_terms.push(t.replace(':', "%3A"));
        }

        let dir_name = workspace_path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase());

        let all = self.list_conversations()?;
        let filtered = all
            .into_iter()
            .filter(|c| {
                let uris_lower = c.workspace_uris.to_lowercase();
                if all_terms.iter().any(|term| {
                    c.workspace_uris.contains(term) || uris_lower.contains(&term.to_lowercase())
                }) {
                    return true;
                }

                if let Some(ref d_name) = dir_name {
                    if !c.project_id.trim().is_empty() && c.project_id.to_lowercase() == *d_name {
                        return true;
                    }
                }

                false
            })
            .collect();

        Ok(filtered)
    }

    pub fn get_conversation(&self, conv_id: &str) -> Result<Option<ConversationSummary>> {
        let conn = self.open_read_only()?;
        let mut stmt = conn.prepare(
            "SELECT 
                conversation_id, title, preview, step_count, last_modified_time, 
                workspace_uris, status, source, project_id, agent_name, 
                parent_conversation_id, nesting_depth, battle_id, winning_conversation_id, 
                not_fully_idle, killed, last_user_input_time, last_user_input_step_index, 
                app_data_dir, raw_summary, group_id
            FROM conversation_summaries 
            WHERE conversation_id = ?1",
        )?;

        let mut rows = stmt.query_map(params![conv_id], |row| {
            Ok(ConversationSummary {
                conversation_id: row.get(0)?,
                title: row.get(1)?,
                preview: row.get(2)?,
                step_count: row.get(3)?,
                last_modified_time: row.get(4)?,
                workspace_uris: row.get(5)?,
                status: row.get(6)?,
                source: row.get(7)?,
                project_id: row.get(8)?,
                agent_name: row.get(9)?,
                parent_conversation_id: row.get(10)?,
                nesting_depth: row.get(11)?,
                battle_id: row.get(12)?,
                winning_conversation_id: row.get(13)?,
                not_fully_idle: row.get::<_, i64>(14)? != 0,
                killed: row.get::<_, i64>(15)? != 0,
                last_user_input_time: row.get(16)?,
                last_user_input_step_index: row.get(17)?,
                app_data_dir: row.get(18)?,
                raw_summary: row.get(19)?,
                group_id: row.get(20)?,
            })
        })?;

        if let Some(first) = rows.next() {
            Ok(Some(first?))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_conversation(&self, conv: &ConversationSummary) -> Result<()> {
        let conn = self.open_read_write()?;
        conn.execute(
            "INSERT OR REPLACE INTO conversation_summaries (
                conversation_id, title, preview, step_count, last_modified_time, 
                workspace_uris, status, source, project_id, agent_name, 
                parent_conversation_id, nesting_depth, battle_id, winning_conversation_id, 
                not_fully_idle, killed, last_user_input_time, last_user_input_step_index, 
                app_data_dir, raw_summary, group_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
            )",
            params![
                conv.conversation_id,
                conv.title,
                conv.preview,
                conv.step_count,
                conv.last_modified_time,
                conv.workspace_uris,
                conv.status,
                conv.source,
                conv.project_id,
                conv.agent_name,
                conv.parent_conversation_id,
                conv.nesting_depth,
                conv.battle_id,
                conv.winning_conversation_id,
                if conv.not_fully_idle { 1 } else { 0 },
                if conv.killed { 1 } else { 0 },
                conv.last_user_input_time,
                conv.last_user_input_step_index,
                conv.app_data_dir,
                conv.raw_summary,
                conv.group_id,
            ],
        )?;

        Ok(())
    }

    pub fn find_project_id_for_workspace(&self, workspace_path: &Path) -> Result<Option<String>> {
        let convs = self.find_conversations_for_workspace(workspace_path)?;
        for c in convs {
            if !c.project_id.trim().is_empty() {
                return Ok(Some(c.project_id));
            }
        }
        Ok(None)
    }

    pub fn upsert_conversation_with_fts(
        &self,
        conv: &ConversationSummary,
        project_name: &str,
        workspace_name: &str,
    ) -> Result<()> {
        self.upsert_conversation(conv)?;

        let conn = self.open_read_write()?;

        let has_fts: bool = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='conversations_fts'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|cnt| cnt > 0)
            .unwrap_or(false);

        if has_fts {
            let now_ns = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let mod_time_ns = chrono::DateTime::parse_from_rfc3339(&conv.last_modified_time)
                .or_else(|_| {
                    chrono::DateTime::parse_from_str(
                        &conv.last_modified_time,
                        "%Y-%m-%d %H:%M:%S%.f%:z",
                    )
                })
                .or_else(|_| {
                    chrono::DateTime::parse_from_str(&conv.last_modified_time, "%Y-%m-%d %H:%M:%S")
                })
                .map(|dt| dt.timestamp_nanos_opt().unwrap_or(now_ns))
                .unwrap_or(now_ns);

            let display_title = if !conv.title.trim().is_empty() {
                conv.title.trim()
            } else if !conv.preview.trim().is_empty() {
                conv.preview.trim()
            } else {
                project_name
            };

            let _ = conn.execute(
                "DELETE FROM conversations_fts WHERE conversation_id = ?1 AND chunk_index IN (-1, 0)",
                params![conv.conversation_id],
            );

            // 1. Chunk -1: Header metadata (title, workspace, project)
            let _ = conn.execute(
                "INSERT INTO conversations_fts (
                    conversation_id, chunk_index, chunk_first_step, chunk_last_step,
                    last_modified_time, step_count, last_step_created_at, archived,
                    user_step_byte_offsets, agent_step_byte_offsets,
                    title, workspace, project, user_body, agent_body
                ) VALUES (
                    ?1, -1, 0, 0,
                    ?2, ?3, ?4, 0,
                    '', '',
                    ?5, ?6, ?7, '', ''
                )",
                params![
                    conv.conversation_id,
                    mod_time_ns,
                    conv.step_count,
                    mod_time_ns,
                    display_title,
                    workspace_name,
                    project_name,
                ],
            );

            // 2. Chunk 0: Initial interaction content (user prompt preview required by Antigravity project list)
            let user_body = if !conv.preview.trim().is_empty() {
                conv.preview.trim()
            } else {
                display_title
            };

            let _ = conn.execute(
                "INSERT INTO conversations_fts (
                    conversation_id, chunk_index, chunk_first_step, chunk_last_step,
                    last_modified_time, step_count, last_step_created_at, archived,
                    user_step_byte_offsets, agent_step_byte_offsets,
                    title, workspace, project, user_body, agent_body
                ) VALUES (
                    ?1, 0, 0, 1,
                    ?2, ?3, ?4, 0,
                    '', '',
                    '', '', '',
                    ?5, ''
                )",
                params![
                    conv.conversation_id,
                    mod_time_ns,
                    conv.step_count,
                    mod_time_ns,
                    user_body,
                ],
            );
        }

        Ok(())
    }
}
