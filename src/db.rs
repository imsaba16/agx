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
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("Failed to open database at {:?} (read-only)", self.db_path))
    }

    fn open_read_write(&self) -> Result<Connection> {
        Connection::open_with_flags(
            &self.db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .with_context(|| format!("Failed to open database at {:?}", self.db_path))
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
}
