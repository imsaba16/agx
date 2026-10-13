use crate::config::AntigravityPaths;
use crate::db::Database;
use crate::extract::Extractor;
use anyhow::{Context, Result};
use colored::Colorize;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::Path;

pub struct DiffEngine;

impl DiffEngine {
    /// Compares a file (defaulting to implementation_plan.md) between two archives
    pub fn diff_bundles(
        bundle_a: &Path,
        bundle_b: &Path,
        file_pattern: Option<&str>,
    ) -> Result<()> {
        let pattern = file_pattern.unwrap_or("plan");

        let (path_a, bytes_a) = Extractor::read_file_content(bundle_a, pattern)
            .with_context(|| format!("Failed to read '{}' from {:?}", pattern, bundle_a))?;
        let (path_b, bytes_b) = Extractor::read_file_content(bundle_b, pattern)
            .with_context(|| format!("Failed to read '{}' from {:?}", pattern, bundle_b))?;

        let text_a = String::from_utf8_lossy(&bytes_a);
        let text_b = String::from_utf8_lossy(&bytes_b);

        let label_a = format!(
            "{:?} ({})",
            bundle_a.file_name().unwrap_or_default(),
            path_a
        );
        let label_b = format!(
            "{:?} ({})",
            bundle_b.file_name().unwrap_or_default(),
            path_b
        );

        Self::print_diff(&text_a, &text_b, &label_a, &label_b);

        Ok(())
    }

    /// Compares a file inside a bundle against the active local workspace's counterpart
    pub fn diff_with_local(
        bundle: &Path,
        workspace_dir: &Path,
        paths: &AntigravityPaths,
        file_pattern: Option<&str>,
    ) -> Result<()> {
        let pattern = file_pattern.unwrap_or("plan");

        let (bundle_path, bundle_bytes) = Extractor::read_file_content(bundle, pattern)
            .with_context(|| format!("Failed to read '{}' from bundle {:?}", pattern, bundle))?;
        let bundle_text = String::from_utf8_lossy(&bundle_bytes);

        // Try finding local active session for this workspace
        let local_text = Self::find_local_content(workspace_dir, paths, pattern)?;

        let label_a = format!(
            "{:?} ({})",
            bundle.file_name().unwrap_or_default(),
            bundle_path
        );
        let label_b = format!(
            "Local Workspace ({:?})",
            workspace_dir.file_name().unwrap_or_default()
        );

        Self::print_diff(&bundle_text, &local_text, &label_a, &label_b);

        Ok(())
    }

    fn find_local_content(
        workspace_dir: &Path,
        paths: &AntigravityPaths,
        pattern: &str,
    ) -> Result<String> {
        let db = Database::new(&paths.summaries_db());
        if paths.summaries_db().exists() {
            let convs = db.find_conversations_for_workspace(workspace_dir)?;
            if let Some(active) = convs.first() {
                let brain_dir = paths.conversation_brain(&active.conversation_id);
                if brain_dir.exists() {
                    let plan_file = brain_dir.join("implementation_plan.md");
                    if pattern == "plan" && plan_file.exists() {
                        return fs::read_to_string(&plan_file).map_err(|e| anyhow::anyhow!(e));
                    }
                    let walkthrough = brain_dir.join("walkthrough.md");
                    if pattern == "walkthrough" && walkthrough.exists() {
                        return fs::read_to_string(&walkthrough).map_err(|e| anyhow::anyhow!(e));
                    }
                }
            }
        }

        // Direct file search in workspace
        let local_direct = workspace_dir.join(pattern);
        if local_direct.exists() {
            return fs::read_to_string(&local_direct).map_err(|e| anyhow::anyhow!(e));
        }

        anyhow::bail!(
            "Could not locate active local session or file matching '{}' for workspace {:?}",
            pattern,
            workspace_dir
        )
    }

    fn print_diff(text_a: &str, text_b: &str, label_a: &str, label_b: &str) {
        println!();
        println!(
            "{} {}",
            "● Context Diff:".bold().cyan(),
            format!("{} ↔ {}", label_a, label_b).white().bold()
        );
        println!(
            "{}",
            "──────────────────────────────────────────────────────────────────".dimmed()
        );

        let diff = TextDiff::from_lines(text_a, text_b);

        let mut additions = 0;
        let mut deletions = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    deletions += 1;
                    print!("{}", format!("- {}", change.value()).red());
                }
                ChangeTag::Insert => {
                    additions += 1;
                    print!("{}", format!("+ {}", change.value()).green());
                }
                ChangeTag::Equal => {
                    print!("{}", format!("  {}", change.value()).dimmed());
                }
            }
        }

        println!();
        println!(
            "{}",
            "──────────────────────────────────────────────────────────────────".dimmed()
        );
        if additions == 0 && deletions == 0 {
            println!(
                "{}",
                "✔ Files are identical (zero changes detected)."
                    .green()
                    .bold()
            );
        } else {
            println!(
                "Diff Stats: {} lines added, {} lines removed",
                additions.to_string().green().bold(),
                deletions.to_string().red().bold()
            );
        }
        println!();
    }
}
