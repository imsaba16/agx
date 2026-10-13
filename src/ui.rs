use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Creates a clean, modern render configuration for `inquire` prompts.
pub fn get_render_config() -> inquire::ui::RenderConfig<'static> {
    inquire::ui::RenderConfig {
        prompt_prefix: inquire::ui::Styled::new("agx ❯")
            .with_fg(inquire::ui::Color::DarkCyan),
        highlighted_option_prefix: inquire::ui::Styled::new("❯ ")
            .with_fg(inquire::ui::Color::DarkCyan),
        scroll_up_prefix: inquire::ui::Styled::new("▲"),
        scroll_down_prefix: inquire::ui::Styled::new("▼"),
        selected_checkbox: inquire::ui::Styled::new("✔ ")
            .with_fg(inquire::ui::Color::DarkGreen),
        unselected_checkbox: inquire::ui::Styled::new("○ ")
            .with_fg(inquire::ui::Color::DarkGrey),
        help_message: inquire::ui::StyleSheet::new()
            .with_fg(inquire::ui::Color::DarkGrey),
        ..Default::default()
    }
}

/// Discovers all `.agbundle` and `.agproject` archives in the target directory
pub fn find_local_archives(dir: &Path) -> Vec<PathBuf> {
    let mut archives = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "agbundle" || ext_lower == "agproject" {
                        archives.push(path);
                    }
                }
            }
        }
    }
    archives.sort();
    archives
}

/// Interactive archive selector that auto-discovers local archives
pub fn select_archive_file(prompt: &str) -> Result<Option<PathBuf>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let local_archives = find_local_archives(&cwd);

    let render_config = get_render_config();

    if !local_archives.is_empty() {
        struct ArchiveItem {
            display: String,
            path: Option<PathBuf>,
        }

        impl std::fmt::Display for ArchiveItem {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.display)
            }
        }

        let mut items = Vec::new();

        for p in &local_archives {
            let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
            let is_proj = filename.ends_with(".agproject");
            let type_label = if is_proj { "Project Archive" } else { "Single Session" };
            let size_kb = std::fs::metadata(p).map(|m| m.len() / 1024).unwrap_or(0);

            items.push(ArchiveItem {
                display: format!("{:<35} {:<16} {:>6} KB", filename, type_label, size_kb),
                path: Some(p.clone()),
            });
        }

        items.push(ArchiveItem {
            display: "Browse or enter custom file path...".to_string(),
            path: None,
        });

        let ans = inquire::Select::new(prompt, items)
            .with_render_config(render_config)
            .with_page_size(10)
            .prompt();

        match ans {
            Ok(selected) => {
                if let Some(p) = selected.path {
                    return Ok(Some(p));
                }
                // Fallback to manual text input
            }
            Err(inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted) => {
                return Ok(None);
            }
            Err(e) => return Err(anyhow::anyhow!("Selection error: {}", e)),
        }
    }

    // Manual text input
    let text_ans = inquire::Text::new("Enter file path:")
        .with_render_config(render_config)
        .prompt();

    match text_ans {
        Ok(s) if !s.trim().is_empty() => Ok(Some(PathBuf::from(s.trim()))),
        _ => Ok(None),
    }
}

/// Render a clean context-aware status banner
pub fn render_context_header(
    workspace_path: &Path,
    active_conversation: Option<&crate::models::ConversationSummary>,
    total_workspace_sessions: usize,
) {
    let ws_display = workspace_path.to_string_lossy();
    let ws_name = workspace_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Workspace".to_string());

    println!();
    println!(
        "{} {}",
        "agx".bold().cyan(),
        "─ Antigravity Context eXchange (v0.1.0)".dimmed()
    );
    println!("  {:<12} : {} ({})", "Workspace".dimmed(), ws_name.bold(), ws_display.dimmed());

    if let Some(conv) = active_conversation {
        let title = if conv.title.trim().is_empty() {
            "Untitled Conversation"
        } else {
            &conv.title
        };
        let date = if conv.last_modified_time.len() >= 16 {
            &conv.last_modified_time[..16]
        } else {
            &conv.last_modified_time
        };
        println!(
            "  {:<12} : {} ({} steps, modified: {})",
            "Active".cyan().bold(),
            title.bold(),
            conv.step_count,
            date.dimmed()
        );
    } else {
        println!(
            "  {:<12} : {}",
            "Active".dimmed(),
            "None matching current directory".dimmed()
        );
    }

    println!(
        "  {:<12} : {} recorded conversations",
        "Sessions".dimmed(),
        total_workspace_sessions.to_string().cyan()
    );
    println!("{}", "──────────────────────────────────────────────────────────────────".dimmed());
}
