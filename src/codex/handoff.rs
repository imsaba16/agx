use anyhow::{Context, Result};
use colored::*;
use std::path::Path;

use crate::config::AntigravityPaths;
use crate::models::ConversationSummary;

pub struct HandoffEngine;

impl HandoffEngine {
    /// Extracts context from an Antigravity session and formats it as a Codex prompt
    pub fn build_handoff_prompt(
        paths: &AntigravityPaths,
        conversation_id: &str,
        workspace_root: &Path,
    ) -> Result<String> {
        let brain_dir = paths.conversation_brain(conversation_id);
        let mut prompt_sections = Vec::new();

        prompt_sections.push(format!(
            "# Context Handoff from Antigravity Session `{}`\n",
            conversation_id
        ));
        prompt_sections.push(format!(
            "Workspace root: `{}`\n",
            workspace_root.display()
        ));

        // 1. Task description
        let task_file = brain_dir.join("task.md");
        if task_file.exists() {
            if let Ok(task_content) = std::fs::read_to_string(&task_file) {
                prompt_sections.push("## Current Task & Objectives\n".to_string());
                prompt_sections.push(task_content.trim().to_string());
                prompt_sections.push("\n".to_string());
            }
        }

        // 2. Implementation plan
        let plan_file = brain_dir.join("plan.md");
        let alt_plan = brain_dir.join("implementation_plan.md");
        let target_plan = if plan_file.exists() {
            Some(plan_file)
        } else if alt_plan.exists() {
            Some(alt_plan)
        } else {
            None
        };

        if let Some(plan_path) = target_plan {
            if let Ok(plan_content) = std::fs::read_to_string(&plan_path) {
                prompt_sections.push("## Active Implementation Plan\n".to_string());
                prompt_sections.push(plan_content.trim().to_string());
                prompt_sections.push("\n".to_string());
            }
        }

        // 3. Recent transcript summary
        let transcript_file = brain_dir
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");

        if transcript_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&transcript_file) {
                let lines: Vec<&str> = content.lines().collect();
                let recent_lines = if lines.len() > 10 {
                    &lines[lines.len() - 10..]
                } else {
                    &lines[..]
                };

                let mut recent_context = Vec::new();
                for l in recent_lines {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(l) {
                        let step_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        let content_txt = val.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        if !content_txt.is_empty() && (step_type == "USER_INPUT" || step_type == "PLANNER_RESPONSE") {
                            recent_context.push(format!("- [{}] {}", step_type, content_txt.chars().take(200).collect::<String>()));
                        }
                    }
                }

                if !recent_context.is_empty() {
                    prompt_sections.push("## Recent Context Turns\n".to_string());
                    prompt_sections.push(recent_context.join("\n"));
                    prompt_sections.push("\n".to_string());
                }
            }
        }

        prompt_sections.push(
            "\nPlease inspect the current repository state and continue implementing the plan from where Antigravity left off.".to_string(),
        );

        Ok(prompt_sections.join("\n"))
    }

    /// Launches Codex with the synthesized handoff prompt
    pub fn launch_codex_interactive(
        codex_binary: &Path,
        workspace_root: &Path,
        prompt: &str,
    ) -> Result<()> {
        println!("{}", "Launching Codex CLI with Antigravity session context...".cyan());

        let mut child = std::process::Command::new(codex_binary)
            .arg("-C")
            .arg(workspace_root)
            .arg(prompt)
            .spawn()
            .with_context(|| format!("Failed to spawn Codex binary at {:?}", codex_binary))?;

        let status = child.wait()?;
        if !status.success() {
            println!("{}", format!("Codex exited with status {}", status).yellow());
        }

        Ok(())
    }

    /// Imports a Codex thread into Antigravity's session registry and brain storage
    pub fn import_codex_thread_to_antigravity(
        paths: &AntigravityPaths,
        thread: &super::db::CodexThread,
    ) -> Result<String> {
        let new_conv_id = format!("cdx-{}", uuid::Uuid::new_v4());
        let brain_dir = paths.conversation_brain(&new_conv_id);
        std::fs::create_dir_all(&brain_dir)?;

        // Write task.md
        let task_content = format!(
            "# Imported Codex Thread: {}\n\nOrigin: Codex CLI\nCreated: {}\nTokens Used: {}\nModel: {}\n\nInitial Prompt:\n{}\n",
            thread.title,
            chrono::DateTime::from_timestamp(thread.created_at, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default(),
            thread.tokens_used,
            thread.model.as_deref().unwrap_or("unknown"),
            thread.first_user_message
        );
        std::fs::write(brain_dir.join("task.md"), task_content)?;

        // Write minimal transcript.jsonl
        let logs_dir = brain_dir.join(".system_generated").join("logs");
        std::fs::create_dir_all(&logs_dir)?;

        let user_entry = serde_json::json!({
            "step_index": 0,
            "type": "USER_INPUT",
            "content": thread.first_user_message,
            "created_at": chrono::DateTime::from_timestamp(thread.created_at, 0).map(|d| d.to_rfc3339()).unwrap_or_default(),
        });
        let assistant_entry = serde_json::json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "content": format!("Thread imported from Codex CLI ({})", thread.title),
            "created_at": chrono::DateTime::from_timestamp(thread.updated_at, 0).map(|d| d.to_rfc3339()).unwrap_or_default(),
        });

        let transcript_content = format!("{}\n{}\n", user_entry, assistant_entry);
        std::fs::write(logs_dir.join("transcript.jsonl"), transcript_content)?;

        // Register into Antigravity SQLite
        let mut summary: ConversationSummary = thread.to_conversation_summary(&paths.base_dir);
        summary.conversation_id = new_conv_id.clone();
        summary.source = "codex_imported".to_string();

        let db = crate::db::Database::new(&paths.summaries_db());
        db.upsert_conversation(&summary)?;

        Ok(new_conv_id)
    }
}
