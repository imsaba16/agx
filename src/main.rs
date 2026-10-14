use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use colored::*;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use std::path::{Path, PathBuf};

use agx::bundler::Bundler;
use agx::config::AntigravityPaths;
use agx::db::Database;
use agx::ide::IdeStorage;
use agx::importer::Importer;
use agx::models::{ExportOptions, ImportOptions, ProjectExportOptions, ProjectImportOptions};
use agx::project_bundler::ProjectBundler;
use agx::project_importer::ProjectImporter;

#[derive(Parser, Debug)]
#[command(name = "agx")]
#[command(author = "Antigravity Developers")]
#[command(version = "1.0.0-rc.1")]
#[command(about = "agx: Antigravity eXtension — Context exchange, session diffing, search, and backup engine for Google Antigravity", long_about = None)]
struct Cli {
    /// Custom path to Antigravity directory (defaults to ~/.gemini/antigravity)
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List all Antigravity conversations and projects
    List {
        /// Filter conversations matching a workspace directory substring
        #[arg(short, long)]
        workspace: Option<String>,

        /// Maximum number of conversations to display
        #[arg(short, long, default_value_t = 15)]
        limit: usize,

        /// Output results in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Export a single conversation and its project brain into a portable .agbundle archive
    Export {
        /// Conversation ID to export (if omitted and inside a workspace, exports the latest for this workspace)
        #[arg(long)]
        id: Option<String>,

        /// Destination bundle file path (e.g. session.agbundle)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Workspace root path to normalize to {{WORKSPACE_ROOT}} (defaults to current working directory)
        #[arg(short, long)]
        workspace_root: Option<PathBuf>,

        /// Sanitize and redact API keys, tokens, and private credentials
        #[arg(long, default_value_t = true)]
        sanitize: bool,

        /// Compact mode: include only core plans, walkthroughs, and transcripts (drops heavy step logs & media)
        #[arg(short, long)]
        compact: bool,

        /// Interactive TUI picker: choose conversation from an interactive menu with fuzzy search
        #[arg(short = 'i', long)]
        interactive: bool,

        /// Path to custom .agignore file (defaults to .agignore in workspace or home)
        #[arg(long)]
        ignore_file: Option<PathBuf>,
    },

    /// Inspect metadata and details of an .agbundle without importing it
    Info {
        /// Path to the .agbundle file
        bundle: PathBuf,

        /// Output bundle metadata in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Import an .agbundle into local Antigravity instance
    Import {
        /// Path to the .agbundle file to import
        bundle: PathBuf,

        /// Target local workspace directory to rebind {{WORKSPACE_ROOT}} to
        #[arg(short, long)]
        target_workspace: Option<PathBuf>,

        /// Overwrite if conversation ID already exists locally (otherwise safely forks with a new ID)
        #[arg(long)]
        overwrite: bool,

        /// Show what would be imported without making any changes
        #[arg(long)]
        dry_run: bool,

        /// Automatically restart Antigravity after import to reflect changes immediately
        #[arg(long, visible_alias = "auto-restart")]
        restart: bool,
    },

    /// Gracefully restart Antigravity to reload projects, conversations, and language server
    Restart {
        /// Optional workspace directory to reopen
        #[arg(short, long)]
        workspace: Option<PathBuf>,
    },

    /// Verify cryptographic integrity (SHA-256) and security safety of an archive (.agbundle or .agproject)
    Verify {
        /// Path to the archive file (.agbundle or .agproject)
        bundle: PathBuf,

        /// Output verification report in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Compare context or implementation plans between two archives or against local workspace
    Diff {
        /// First archive to compare (.agbundle or .agproject)
        bundle_a: PathBuf,

        /// Second archive to compare against (optional if comparing against local workspace)
        bundle_b: Option<PathBuf>,

        /// Specific file pattern to diff (defaults to 'plan' / implementation_plan.md)
        #[arg(short, long)]
        file: Option<String>,

        /// Compare bundle_a against the active local workspace's counterpart
        #[arg(long)]
        local: bool,
    },

    /// Search across archived bundles or local sessions without decompressing to disk
    Search {
        /// Search query or keyword
        query: String,

        /// Directory containing archives to search (defaults to current directory)
        #[arg(short, long)]
        dir: Option<PathBuf>,

        /// Also search across local Antigravity brain storage
        #[arg(long)]
        local: bool,

        /// Case-sensitive matching
        #[arg(short = 'c', long)]
        case_sensitive: bool,
    },

    /// Surgically extract specific files (e.g. plan, walkthrough) from an archive without importing
    Extract {
        /// Path to the archive file (.agbundle or .agproject)
        bundle: PathBuf,

        /// File pattern to extract (e.g. 'plan', 'walkthrough', 'transcript', or filename)
        #[arg(short, long)]
        pattern: Option<String>,

        /// Destination output file or folder
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Print contents of an archived file directly to stdout (ideal for pipes, grep, or pager)
    Cat {
        /// Path to the archive file (.agbundle or .agproject)
        bundle: PathBuf,

        /// File to print (e.g. 'plan', 'walkthrough', 'manifest', 'transcript')
        pattern: String,
    },

    /// Launch full-screen interactive split-pane TUI explorer & markdown previewer
    Explore,

    /// Whole-project operations: export/import all sessions, brain artifacts, skills, and MCP configs
    Project {
        #[command(subcommand)]
        action: ProjectCommands,
    },

    /// Generate shell autocompletion scripts (bash, zsh, fish, powershell, elvish)
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectCommands {
    /// Export an entire project (all conversations, brain artifacts, skills, and MCP) into a .agproject archive
    Export {
        /// Project workspace root directory (defaults to current directory)
        #[arg(short, long)]
        workspace: Option<PathBuf>,

        /// Destination output file path (e.g. my_project.agproject)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Compact mode: include only core plans & compact transcripts for all sessions (drops raw step logs)
        #[arg(short, long)]
        compact: bool,

        /// Interactive TUI: select which sessions to include in the project bundle
        #[arg(short = 'i', long)]
        interactive: bool,

        /// Skip exporting project customizations (.agents/skills, rules, MCP)
        #[arg(long)]
        no_customizations: bool,

        /// Sanitize secrets, API keys, and private credentials
        #[arg(long, default_value_t = true)]
        sanitize: bool,

        /// Path to custom .agignore file
        #[arg(long)]
        ignore_file: Option<PathBuf>,
    },

    /// Inspect metadata, session list, and customizations of an .agproject archive
    Info {
        /// Path to the .agproject bundle file
        bundle: PathBuf,

        /// Output project manifest in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Import an .agproject bundle into local Antigravity instance, restoring all sessions and customizations
    Import {
        /// Path to the .agproject bundle file to import
        bundle: PathBuf,

        /// Target local workspace directory to rebind the project to
        #[arg(short, long)]
        target_workspace: Option<PathBuf>,

        /// Overwrite existing sessions and project customizations if present
        #[arg(long)]
        overwrite: bool,

        /// Skip restoring workspace customizations (.agents/skills, rules, MCP)
        #[arg(long)]
        no_customizations: bool,

        /// Show what would be imported without making any changes
        #[arg(long)]
        dry_run: bool,

        /// Automatically restart Antigravity after import to reflect changes immediately
        #[arg(long, visible_alias = "auto-restart")]
        restart: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AntigravityPaths::discover(cli.data_dir)?;

    match cli.command {
        Some(Commands::List {
            workspace,
            limit,
            json,
        }) => {
            handle_list(&paths, workspace, limit, json)?;
        }
        Some(Commands::Export {
            id,
            output,
            workspace_root,
            sanitize,
            compact,
            interactive,
            ignore_file,
        }) => {
            handle_export(
                &paths,
                id,
                output,
                workspace_root,
                sanitize,
                compact,
                interactive,
                ignore_file,
            )?;
        }
        Some(Commands::Info { bundle, json }) => {
            handle_info(&bundle, json)?;
        }
        Some(Commands::Restart { workspace }) => {
            handle_restart(workspace)?;
        }
        Some(Commands::Import {
            bundle,
            target_workspace,
            overwrite,
            dry_run,
            restart,
        }) => {
            handle_import(
                &paths,
                bundle,
                target_workspace,
                overwrite,
                dry_run,
                restart,
            )?;
        }
        Some(Commands::Verify { bundle, json }) => {
            handle_verify(&bundle, json)?;
        }
        Some(Commands::Diff {
            bundle_a,
            bundle_b,
            file,
            local,
        }) => {
            if local || bundle_b.is_none() {
                let ws = std::env::current_dir()?;
                agx::diff::DiffEngine::diff_with_local(&bundle_a, &ws, &paths, file.as_deref())?;
            } else if let Some(b) = bundle_b {
                agx::diff::DiffEngine::diff_bundles(&bundle_a, &b, file.as_deref())?;
            }
        }
        Some(Commands::Search {
            query,
            dir,
            local,
            case_sensitive,
        }) => {
            let search_dir = dir.unwrap_or_else(|| PathBuf::from("."));
            let archive_hits =
                agx::search::SearchEngine::search_archives(&query, &search_dir, case_sensitive)?;
            let mut total_hits = archive_hits;

            if local {
                let local_hits =
                    agx::search::SearchEngine::search_local(&query, &paths, case_sensitive)?;
                total_hits.extend(local_hits);
            }

            agx::search::SearchEngine::print_hits(&query, &total_hits);
        }
        Some(Commands::Extract {
            bundle,
            pattern,
            output,
        }) => {
            let extracted =
                agx::extract::Extractor::extract(&bundle, pattern.as_deref(), output.as_deref())?;
            println!(
                "\n{}",
                format!("✔ Successfully extracted {} file(s):", extracted.len())
                    .green()
                    .bold()
            );
            for p in extracted {
                println!("  - {:?}", p);
            }
            println!();
        }
        Some(Commands::Cat { bundle, pattern }) => {
            let (_, bytes) = agx::extract::Extractor::read_file_content(&bundle, &pattern)?;
            let text = String::from_utf8_lossy(&bytes);
            print!("{}", text);
        }
        Some(Commands::Explore) => {
            agx::explorer::run_explorer(&paths)?;
        }
        Some(Commands::Project { action }) => match action {
            ProjectCommands::Export {
                workspace,
                output,
                compact,
                interactive,
                no_customizations,
                sanitize,
                ignore_file,
            } => {
                handle_project_export(
                    &paths,
                    workspace,
                    output,
                    compact,
                    interactive,
                    !no_customizations,
                    sanitize,
                    ignore_file,
                )?;
            }
            ProjectCommands::Info { bundle, json } => {
                handle_project_info(&bundle, json)?;
            }
            ProjectCommands::Import {
                bundle,
                target_workspace,
                overwrite,
                no_customizations,
                dry_run,
                restart,
            } => {
                handle_project_import(
                    &paths,
                    bundle,
                    target_workspace,
                    overwrite,
                    !no_customizations,
                    dry_run,
                    restart,
                )?;
            }
        },
        Some(Commands::Completion { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "agx", &mut std::io::stdout());
        }
        None => {
            run_interactive_dashboard(&paths)?;
        }
    }

    Ok(())
}

fn run_interactive_dashboard(paths: &AntigravityPaths) -> Result<()> {
    let render_config = agx::ui::get_render_config();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let db = Database::new(&paths.summaries_db());
    let (active_conv, ws_sessions_count) = if paths.summaries_db().exists() {
        let ws_convs = db
            .find_conversations_for_workspace(&cwd)
            .unwrap_or_default();
        let count = ws_convs.len();
        let active = ws_convs.into_iter().next();
        (active, count)
    } else {
        (None, 0)
    };

    agx::ui::render_context_header(&cwd, active_conv.as_ref(), ws_sessions_count);

    struct ActionItem {
        id: &'static str,
        title: &'static str,
        desc: &'static str,
    }

    impl std::fmt::Display for ActionItem {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:<32} {}", self.title, self.desc.dimmed())
        }
    }

    let actions = [
        ActionItem {
            id: "explore",
            title: "Split-Screen TUI Explorer",
            desc: "Interactive session navigator & live preview",
        },
        ActionItem {
            id: "export_active",
            title: "Export active session",
            desc: "Package current conversation into .agbundle",
        },
        ActionItem {
            id: "export_project",
            title: "Export entire project (.agproject)",
            desc: "All conversations, brain files, skills & MCP",
        },
        ActionItem {
            id: "browse_export",
            title: "Browse & export past sessions",
            desc: "Interactive session picker with search",
        },
        ActionItem {
            id: "diff",
            title: "Diff context against local/archive",
            desc: "Visual side-by-side plan changes",
        },
        ActionItem {
            id: "search",
            title: "Search session contents",
            desc: "Zero-decompression grep across archives",
        },
        ActionItem {
            id: "extract",
            title: "Extract specific artifact",
            desc: "Extract plan, walkthrough, or transcript",
        },
        ActionItem {
            id: "inspect",
            title: "Inspect archive metadata",
            desc: "View manifest, steps, and artifacts",
        },
        ActionItem {
            id: "verify",
            title: "Verify archive integrity",
            desc: "Cryptographic SHA-256 audit & security check",
        },
        ActionItem {
            id: "import",
            title: "Import archive into workspace",
            desc: "Restore .agbundle or .agproject into Antigravity",
        },
        ActionItem {
            id: "list",
            title: "List workspace sessions",
            desc: "View formatted conversation history table",
        },
        ActionItem {
            id: "restart",
            title: "Restart Antigravity",
            desc: "Reload projects, conversations, and language server",
        },
        ActionItem {
            id: "exit",
            title: "Exit",
            desc: "Close agx",
        },
    ];

    loop {
        let selection = match inquire::Select::new("Select an action:", actions.iter().collect())
            .with_render_config(render_config)
            .with_page_size(12)
            .prompt()
        {
            Ok(s) => s.id,
            Err(_) => break,
        };

        match selection {
            "explore" => {
                let _ = agx::explorer::run_explorer(paths);
            }
            "export_active" => {
                let compact_prompt = inquire::Select::new(
                    "Select export mode:",
                    vec![
                        "Compact Mode  (Fast, lightweight, skips heavy raw step logs & media)",
                        "Full Fidelity (Complete raw step logs and heavy media)",
                    ],
                )
                .with_render_config(render_config)
                .prompt();

                match compact_prompt {
                    Ok(choice) => {
                        let is_compact = choice.starts_with("Compact");
                        let _ =
                            handle_export(paths, None, None, None, true, is_compact, false, None);
                    }
                    _ => continue,
                }
            }
            "export_project" => {
                let compact_prompt = inquire::Select::new(
                    "Select project export mode:",
                    vec![
                        "Full Fidelity (All conversations, complete logs, skills & MCP)",
                        "Compact Mode  (Stripped raw step logs across all sessions)",
                    ],
                )
                .with_render_config(render_config)
                .prompt();

                match compact_prompt {
                    Ok(choice) => {
                        let is_compact = choice.starts_with("Compact");
                        let _ = handle_project_export(
                            paths, None, None, is_compact, true, true, true, None,
                        );
                    }
                    _ => continue,
                }
            }
            "browse_export" => {
                let _ = handle_export(paths, None, None, None, true, false, true, None);
            }
            "diff" => {
                if let Ok(Some(archive_path)) =
                    agx::ui::select_archive_file("Select archive to compare:")
                {
                    let _ =
                        agx::diff::DiffEngine::diff_with_local(&archive_path, &cwd, paths, None);
                }
            }
            "search" => {
                let query_ans = inquire::Text::new("Enter search query or keyword:")
                    .with_render_config(render_config)
                    .prompt();
                if let Ok(query) = query_ans {
                    if !query.trim().is_empty() {
                        let mut hits =
                            agx::search::SearchEngine::search_archives(query.trim(), &cwd, false)?;
                        let local_hits =
                            agx::search::SearchEngine::search_local(query.trim(), paths, false)?;
                        hits.extend(local_hits);
                        agx::search::SearchEngine::print_hits(query.trim(), &hits);
                    }
                }
            }
            "extract" => {
                if let Ok(Some(archive_path)) =
                    agx::ui::select_archive_file("Select archive to extract from:")
                {
                    let pattern_choice = inquire::Select::new(
                        "Select file to extract:",
                        vec![
                            "Implementation Plan (implementation_plan.md)",
                            "Walkthrough (walkthrough.md)",
                            "Transcript (transcript.jsonl)",
                            "Manifest (manifest.json)",
                            "Extract all files",
                        ],
                    )
                    .with_render_config(render_config)
                    .prompt();

                    let pattern = match pattern_choice {
                        Ok("Implementation Plan (implementation_plan.md)") => Some("plan"),
                        Ok("Walkthrough (walkthrough.md)") => Some("walkthrough"),
                        Ok("Transcript (transcript.jsonl)") => Some("transcript"),
                        Ok("Manifest (manifest.json)") => Some("manifest"),
                        _ => None,
                    };

                    let extracted = agx::extract::Extractor::extract(&archive_path, pattern, None)?;
                    println!(
                        "\n{}",
                        format!(
                            "✔ Successfully extracted {} file(s) into current folder:",
                            extracted.len()
                        )
                        .green()
                        .bold()
                    );
                    for p in extracted {
                        println!("  - {:?}", p);
                    }
                }
            }
            "inspect" => {
                if let Ok(Some(archive_path)) =
                    agx::ui::select_archive_file("Select archive to inspect:")
                {
                    if is_project_archive(&archive_path) {
                        let _ = handle_project_info(&archive_path, false);
                    } else {
                        let _ = handle_info(&archive_path, false);
                    }
                }
            }
            "verify" => {
                if let Ok(Some(archive_path)) =
                    agx::ui::select_archive_file("Select archive to verify:")
                {
                    let _ = handle_verify(&archive_path, false);
                }
            }
            "import" => {
                if let Ok(Some(archive_path)) =
                    agx::ui::select_archive_file("Select archive to import:")
                {
                    let ws_path_str = match inquire::Text::new(
                        "Target workspace path (press Enter for current folder):",
                    )
                    .with_render_config(render_config)
                    .prompt()
                    {
                        Ok(s) => s.trim().to_string(),
                        _ => String::new(),
                    };
                    let target_ws = if ws_path_str.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(ws_path_str))
                    };

                    let imported_ok = if is_project_archive(&archive_path) {
                        handle_project_import(
                            paths,
                            archive_path,
                            target_ws.clone(),
                            false,
                            true,
                            false,
                            false,
                        )
                        .is_ok()
                    } else {
                        handle_import(
                            paths,
                            archive_path,
                            target_ws.clone(),
                            false,
                            false,
                            false,
                        )
                        .is_ok()
                    };

                    if imported_ok && IdeStorage::is_antigravity_running() {
                        let restart_choice = inquire::Confirm::new(
                            "Antigravity is currently running. Would you like to restart it now to apply changes immediately?",
                        )
                        .with_default(true)
                        .with_render_config(render_config)
                        .prompt();

                        if let Ok(true) = restart_choice {
                            println!("{}", "● Restarting Antigravity...".cyan());
                            match IdeStorage::restart_antigravity(target_ws.as_deref()) {
                                Ok(_) => println!(
                                    "{}",
                                    "✔ Successfully restarted Antigravity! Changes are now active."
                                        .green()
                                        .bold()
                                ),
                                Err(e) => {
                                    eprintln!("{}", format!("⚠ Could not restart Antigravity: {}", e).yellow())
                                }
                            }
                        }
                    }
                }
            }
            "list" => {
                let _ = handle_list(paths, None, 20, false);
            }
            "restart" => {
                let _ = handle_restart(Some(cwd.clone()));
            }
            "exit" => break,
            _ => break,
        }

        println!();
    }

    Ok(())
}

fn handle_list(
    paths: &AntigravityPaths,
    workspace_filter: Option<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    if !paths.summaries_db().exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Antigravity database not found at {:?}",
                paths.summaries_db()
            )
            .red()
        );
        std::process::exit(1);
    }

    let db = Database::new(&paths.summaries_db());
    let conversations = db.list_conversations()?;

    let filtered: Vec<_> = conversations
        .into_iter()
        .filter(|c| {
            if let Some(ref filter) = workspace_filter {
                c.workspace_uris
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
            } else {
                true
            }
        })
        .take(limit)
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    if filtered.is_empty() {
        println!("{}", "No matching conversations found.".yellow());
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Title").add_attribute(Attribute::Bold),
            Cell::new("Modified").add_attribute(Attribute::Bold),
            Cell::new("Steps").add_attribute(Attribute::Bold),
            Cell::new("Brain").add_attribute(Attribute::Bold),
            Cell::new("DB").add_attribute(Attribute::Bold),
        ]);

    for conv in &filtered {
        let brain_exists = paths.conversation_brain(&conv.conversation_id).exists();
        let db_exists = paths.conversation_db(&conv.conversation_id).exists();

        let display_title = if conv.title.trim().is_empty() {
            "Untitled Conversation"
        } else {
            &conv.title
        };

        let short_id = if conv.conversation_id.len() > 8 {
            &conv.conversation_id[..8]
        } else {
            &conv.conversation_id
        };

        let date = if conv.last_modified_time.len() >= 19 {
            conv.last_modified_time[..19].replace('T', " ")
        } else {
            conv.last_modified_time.clone()
        };

        table.add_row(vec![
            Cell::new(short_id).fg(Color::DarkGrey),
            Cell::new(display_title).add_attribute(Attribute::Bold),
            Cell::new(date).fg(Color::Cyan),
            Cell::new(conv.step_count.to_string()).fg(Color::Yellow),
            if brain_exists {
                Cell::new("✓").fg(Color::Green)
            } else {
                Cell::new("-").fg(Color::DarkGrey)
            },
            if db_exists {
                Cell::new("✓").fg(Color::Green)
            } else {
                Cell::new("-").fg(Color::DarkGrey)
            },
        ]);
    }

    println!("\n{}", "=== Antigravity Conversations ===".bold().cyan());
    println!("{table}");
    println!("Total displayed: {}\n", filtered.len());

    Ok(())
}

fn select_conversation_interactively(
    conversations: &[agx::models::ConversationSummary],
) -> Result<agx::models::ConversationSummary> {
    if conversations.is_empty() {
        anyhow::bail!("No conversations found in Antigravity database.");
    }

    struct Item<'a> {
        summary: &'a agx::models::ConversationSummary,
        display: String,
    }

    impl<'a> std::fmt::Display for Item<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.display)
        }
    }

    let items: Vec<Item> = conversations
        .iter()
        .map(|c| {
            let date = if c.last_modified_time.len() >= 10 {
                &c.last_modified_time[..10]
            } else {
                &c.last_modified_time
            };

            let title = if c.title.trim().is_empty() {
                "Untitled Conversation"
            } else {
                &c.title
            };

            let display = format!("[{}] {} ({} steps)", date, title, c.step_count);

            Item {
                summary: c,
                display,
            }
        })
        .collect();

    let ans = inquire::Select::new("Select conversation to export:", items)
        .with_render_config(agx::ui::get_render_config())
        .with_page_size(15)
        .with_help_message("Use ↑/↓ arrows to navigate, type to filter, Esc to cancel")
        .prompt();

    match ans {
        Ok(selected) => Ok(selected.summary.clone()),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            println!("{}", "Export cancelled by user.".yellow());
            std::process::exit(0);
        }
        Err(e) => Err(anyhow::anyhow!("Interactive selection failed: {}", e)),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_export(
    paths: &AntigravityPaths,
    conv_id_opt: Option<String>,
    output_path: Option<PathBuf>,
    workspace_root: Option<PathBuf>,
    sanitize: bool,
    compact: bool,
    interactive: bool,
    ignore_file: Option<PathBuf>,
) -> Result<()> {
    let db = Database::new(&paths.summaries_db());

    // Resolve conversation ID and summary
    let (conv_id, summary) = if let Some(id) = conv_id_opt {
        let summary = db
            .get_conversation(&id)?
            .with_context(|| format!("Conversation ID '{}' not found in database", id))?;
        (id, summary)
    } else if interactive {
        let list = db.list_conversations()?;
        let selected = select_conversation_interactively(&list)?;
        (selected.conversation_id.clone(), selected)
    } else {
        // Auto-detect by current directory or prompt interactively if inside terminal
        let cwd = std::env::current_dir()?;
        let cwd_str = cwd.to_string_lossy();
        let list = db.list_conversations()?;
        let matching = list
            .into_iter()
            .find(|c| c.workspace_uris.contains(&*cwd_str));

        if let Some(conv) = matching {
            println!(
                "{}",
                format!(
                    "Auto-detected active conversation for current directory: {}",
                    conv.title
                )
                .green()
            );
            (conv.conversation_id.clone(), conv)
        } else {
            let all = db.list_conversations()?;
            println!(
                "{}",
                "No active conversation matched current directory. Opening interactive picker..."
                    .yellow()
            );
            let selected = select_conversation_interactively(&all)?;
            (selected.conversation_id.clone(), selected)
        }
    };

    let ws_root = workspace_root.or_else(|| std::env::current_dir().ok());

    let final_output = output_path.unwrap_or_else(|| {
        let sanitized_title: String = summary
            .title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let suffix = if compact { "_compact" } else { "" };
        PathBuf::from(format!(
            "{}{}_{}.agbundle",
            sanitized_title,
            suffix,
            &conv_id[..8]
        ))
    });

    let options = ExportOptions {
        conversation_id: conv_id.clone(),
        output_path: final_output.clone(),
        sanitize,
        compact,
        workspace_root: ws_root.clone(),
        custom_ignore_file: ignore_file,
    };

    println!(
        "\n{}",
        "● Packaging Session Archive (.agbundle)".bold().cyan()
    );
    println!("  Title       : {}", summary.title.bold());
    println!("  ID          : {}", conv_id);
    println!(
        "  Sanitize    : {}",
        if sanitize {
            "YES (Masking secrets)".green()
        } else {
            "NO".yellow()
        }
    );
    println!(
        "  Compact Mode: {}",
        if compact {
            "YES (Skipping raw step logs & heavy media)".green()
        } else {
            "NO (Full fidelity)".dimmed()
        }
    );
    if let Some(ref ws) = ws_root {
        println!("  Remap Root  : {:?}", ws);
    }
    println!("  Output File : {:?}", final_output);

    let manifest = Bundler::export_bundle(paths, summary, &options)?;

    println!(
        "\n{}",
        format!(
            "✔ Successfully exported session archive! Packaged {} brain files ({} files excluded by .agignore/compact filter).",
            manifest.artifacts_count,
            manifest.ignored_count
        )
        .bold()
        .green()
    );
    println!("Archive created at: {:?}\n", final_output);

    Ok(())
}

fn handle_info(bundle_path: &Path, json: bool) -> Result<()> {
    let manifest = Bundler::inspect_bundle(bundle_path)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
        return Ok(());
    }

    println!(
        "\n{}",
        "=== Antigravity Bundle Information ===".bold().cyan()
    );
    println!("  Bundle Version : {}", manifest.version.yellow());
    println!("  Created At     : {}", manifest.created_at.cyan());
    println!(
        "  Source Host    : {} ({})",
        manifest.source_username, manifest.source_os
    );
    println!(
        "  Sanitized      : {}",
        if manifest.sanitized {
            "YES".green()
        } else {
            "NO".yellow()
        }
    );
    println!(
        "  Compact Mode   : {}",
        if manifest.compact {
            "YES".green()
        } else {
            "NO".dimmed()
        }
    );
    println!(
        "  Artifacts Count: {}",
        manifest.artifacts_count.to_string().bold()
    );
    println!(
        "  Excluded Count : {}",
        manifest.ignored_count.to_string().yellow()
    );
    println!(
        "  Includes DB    : {}",
        if manifest.has_db {
            "YES".green()
        } else {
            "NO".red()
        }
    );
    println!("\n{}", "--- Conversation Details ---".bold());
    println!(
        "  Title          : {}",
        manifest.conversation.title.bold().white()
    );
    println!(
        "  Original ID    : {}",
        manifest.conversation.conversation_id
    );
    println!("  Step Count     : {}", manifest.conversation.step_count);
    println!("  Original Roots : {:?}", manifest.original_workspace_paths);
    println!();

    Ok(())
}

fn handle_import(
    paths: &AntigravityPaths,
    bundle_path: PathBuf,
    target_workspace: Option<PathBuf>,
    overwrite: bool,
    dry_run: bool,
    restart: bool,
) -> Result<()> {
    let options = ImportOptions {
        bundle_path: bundle_path.clone(),
        target_workspace: target_workspace.or_else(|| std::env::current_dir().ok()),
        overwrite,
        dry_run,
    };

    println!("\n{}", "● Importing Session Archive...".bold().cyan());
    if dry_run {
        println!(
            "{}",
            "MODE: DRY-RUN (No files will be modified)".yellow().bold()
        );
    }

    let (imported_id, manifest) = Importer::import_bundle(paths, &options)?;

    println!("  Title           : {}", manifest.conversation.title.bold());
    println!(
        "  Original ID     : {}",
        manifest.conversation.conversation_id
    );
    println!("  Assigned ID     : {}", imported_id.cyan());
    if let Some(ref target_ws) = options.target_workspace {
        println!("  Bound Workspace : {:?}", target_ws);
    }
    println!("  Artifacts Count : {}", manifest.artifacts_count);

    if dry_run {
        println!(
            "\n{}",
            "Dry-run complete. Run without --dry-run to execute import.".green()
        );
    } else {
        println!(
            "\n{}",
            "✔ Successfully imported into Antigravity! Ready to use in chat or IDE."
                .bold()
                .green()
        );

        if restart {
            if IdeStorage::is_antigravity_running() {
                println!(
                    "{}",
                    "● Restarting Antigravity to reload projects and language server...".cyan()
                );
                match IdeStorage::restart_antigravity(options.target_workspace.as_deref()) {
                    Ok(_) => println!(
                        "{}",
                        "✔ Successfully restarted Antigravity! Changes are now active."
                            .green()
                            .bold()
                    ),
                    Err(e) => {
                        eprintln!("{}", format!("⚠ Failed to restart Antigravity: {}", e).yellow())
                    }
                }
            } else {
                println!(
                    "{}",
                    "Antigravity is not currently running. Sessions will be loaded on next startup."
                        .dimmed()
                );
            }
        } else if IdeStorage::is_antigravity_running() {
            println!(
                "{}",
                "💡 Tip: Antigravity is currently running. Run 'agx restart' (or pass --restart) to view your imported session immediately."
                    .dimmed()
            );
        }
    }
    println!();

    Ok(())
}

fn handle_verify(bundle_path: &Path, json: bool) -> Result<()> {
    if !bundle_path.exists() {
        anyhow::bail!("Bundle file does not exist: {:?}", bundle_path);
    }

    let is_project = is_project_archive(bundle_path);

    let (report, archive_type) = if is_project {
        (
            ProjectBundler::verify_project_bundle(bundle_path)?,
            "Project Archive (.agproject)",
        )
    } else {
        (
            Bundler::verify_bundle(bundle_path)?,
            "Conversation Bundle (.agbundle)",
        )
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.is_valid {
            std::process::exit(1);
        }
        return Ok(());
    }

    println!("\n{}", "● AGX Security & Integrity Audit".bold().cyan());
    println!(
        "{}",
        "────────────────────────────────────────────────".dimmed()
    );
    println!("  Archive File   : {:?}", bundle_path);
    println!("  Archive Format : {}", archive_type.bold());
    println!(
        "  Files Checked  : {}",
        report.total_files_checked.to_string().cyan()
    );
    println!(
        "  Valid Hashes   : {}",
        report.matched_checksums.to_string().green().bold()
    );

    if report.is_valid {
        println!("\n{}", "✔ INTEGRITY AUDIT PASSED".bold().green());
        println!("  • SHA-256 Checksums : Verified all recorded files match cryptographic hashes.");
        println!("  • Path Traversal    : No Zip-Slip or illegal path traversal vectors detected.");
        println!("  • Resource Limits   : Decompression bomb safeguards enforced & verified.");
        println!("  • File Modes        : Sanitized permissions (0o644 / 0o755).\n");
    } else {
        println!("\n{}", "✖ INTEGRITY AUDIT FAILED".bold().red());
        if !report.mismatched_files.is_empty() {
            println!(
                "\n{}",
                "  Mismatched Checksums (File Tampering Detected):"
                    .red()
                    .bold()
            );
            for f in &report.mismatched_files {
                println!("    - {}", f.red());
            }
        }
        if !report.missing_files.is_empty() {
            println!(
                "\n{}",
                "  Missing Files (Declared in manifest but not found):"
                    .yellow()
                    .bold()
            );
            for f in &report.missing_files {
                println!("    - {}", f.yellow());
            }
        }
        if !report.errors.is_empty() {
            println!("\n{}", "  Security / Malformation Errors:".red().bold());
            for err in &report.errors {
                println!("    - {}", err.red());
            }
        }
        println!();
        std::process::exit(1);
    }

    Ok(())
}

fn is_project_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("agproject") {
            return true;
        }
    }
    if let Ok(mut archive) = agx::bundler::open_archive(path) {
        if let Ok(entries) = archive.entries() {
            for entry in entries.flatten() {
                if let Ok(p) = entry.path() {
                    if p.to_string_lossy() == "project_manifest.json" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn select_project_conversations_interactively(
    conversations: &[agx::models::ConversationSummary],
) -> Result<Vec<String>> {
    if conversations.is_empty() {
        anyhow::bail!("No conversations found for project.");
    }

    struct Item<'a> {
        summary: &'a agx::models::ConversationSummary,
        display: String,
    }

    impl<'a> std::fmt::Display for Item<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.display)
        }
    }

    let items: Vec<Item> = conversations
        .iter()
        .map(|c| {
            let date = if c.last_modified_time.len() >= 10 {
                &c.last_modified_time[..10]
            } else {
                &c.last_modified_time
            };
            let title = if c.title.trim().is_empty() {
                "Untitled Conversation"
            } else {
                &c.title
            };
            Item {
                summary: c,
                display: format!("[{}] {} ({} steps)", date, title, c.step_count),
            }
        })
        .collect();

    let ans =
        inquire::MultiSelect::new("Select conversations to include in project archive:", items)
            .with_render_config(agx::ui::get_render_config())
            .with_all_selected_by_default()
            .with_page_size(15)
            .with_help_message(
                "Use Space to toggle, ↑/↓ to navigate, type to filter, Enter to confirm",
            )
            .prompt();

    match ans {
        Ok(selected) => {
            if selected.is_empty() {
                anyhow::bail!("At least one conversation must be selected.");
            }
            Ok(selected
                .into_iter()
                .map(|item| item.summary.conversation_id.clone())
                .collect())
        }
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            println!("{}", "Project export cancelled by user.".yellow());
            std::process::exit(0);
        }
        Err(e) => Err(anyhow::anyhow!("Interactive selection failed: {}", e)),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_project_export(
    paths: &AntigravityPaths,
    workspace: Option<PathBuf>,
    output: Option<PathBuf>,
    compact: bool,
    interactive: bool,
    include_customizations: bool,
    sanitize: bool,
    ignore_file: Option<PathBuf>,
) -> Result<()> {
    let ws_root = workspace
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let db = Database::new(&paths.summaries_db());
    let conversations = db.find_conversations_for_workspace(&ws_root)?;

    if conversations.is_empty() {
        anyhow::bail!(
            "No conversations found associated with workspace {:?}",
            ws_root
        );
    }

    let session_ids = if interactive {
        Some(select_project_conversations_interactively(&conversations)?)
    } else {
        None
    };

    let project_name = ws_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let suffix = if compact { "_compact" } else { "" };
    let final_output =
        output.unwrap_or_else(|| PathBuf::from(format!("{}{}.agproject", project_name, suffix)));

    let options = ProjectExportOptions {
        workspace_root: ws_root.clone(),
        output_path: final_output.clone(),
        session_ids,
        include_customizations,
        sanitize,
        compact,
        custom_ignore_file: ignore_file,
    };

    println!(
        "\n{}",
        "● Packaging Project Archive (.agproject)".bold().cyan()
    );
    println!("  Workspace Root : {:?}", ws_root);
    println!("  Project Name   : {}", project_name.bold());
    println!(
        "  Customizations : {}",
        if include_customizations {
            "YES (.agents/skills, rules, MCP)".green()
        } else {
            "NO (Skipped)".yellow()
        }
    );
    println!(
        "  Sanitize       : {}",
        if sanitize {
            "YES (Masking secrets)".green()
        } else {
            "NO".yellow()
        }
    );
    println!(
        "  Compact Mode   : {}",
        if compact {
            "YES (Skipping raw step logs & heavy media)".green()
        } else {
            "NO (Full fidelity)".dimmed()
        }
    );
    println!("  Output File    : {:?}", final_output);

    let manifest = ProjectBundler::export_project_bundle(paths, &options)?;

    println!(
        "\n{}",
        format!(
            "✔ Successfully exported project archive! Packaged {} conversations, {} brain artifacts ({} excluded).",
            manifest.conversations.len(),
            manifest.total_artifacts_count,
            manifest.total_ignored_count,
        )
        .bold()
        .green()
    );
    if manifest.customizations.has_skills {
        println!(
            "  Skills Included : {} skills",
            manifest.customizations.skills_count
        );
    }
    if manifest.customizations.has_rules {
        println!("  Rules Included  : YES");
    }
    if manifest.customizations.has_mcp {
        println!("  MCP Included    : YES");
    }
    println!("Archive created at: {:?}\n", final_output);

    Ok(())
}

fn handle_project_info(bundle_path: &Path, json: bool) -> Result<()> {
    let manifest = ProjectBundler::inspect_project_bundle(bundle_path)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
        return Ok(());
    }

    println!(
        "\n{}",
        "=== Antigravity Project Bundle Information ==="
            .bold()
            .cyan()
    );
    println!(
        "  Project Name   : {}",
        manifest.project_name.bold().white()
    );
    println!("  Bundle Version : {}", manifest.version.yellow());
    println!("  Created At     : {}", manifest.created_at.cyan());
    println!(
        "  Source Host    : {} ({})",
        manifest.source_username, manifest.source_os
    );
    println!("  Original Path  : {}", manifest.original_workspace_path);
    println!(
        "  Sanitized      : {}",
        if manifest.sanitized {
            "YES".green()
        } else {
            "NO".yellow()
        }
    );
    println!(
        "  Compact Mode   : {}",
        if manifest.compact {
            "YES".green()
        } else {
            "NO".dimmed()
        }
    );
    println!(
        "  Artifacts Count: {}",
        manifest.total_artifacts_count.to_string().bold()
    );
    println!(
        "  Excluded Count : {}",
        manifest.total_ignored_count.to_string().yellow()
    );

    println!("\n{}", "--- Customizations Included ---".bold());
    println!(
        "  Skills : {}",
        if manifest.customizations.has_skills {
            format!("YES ({} skills)", manifest.customizations.skills_count).green()
        } else {
            "NO".dimmed()
        }
    );
    println!(
        "  Rules  : {}",
        if manifest.customizations.has_rules {
            "YES".green()
        } else {
            "NO".dimmed()
        }
    );
    println!(
        "  MCP    : {}",
        if manifest.customizations.has_mcp {
            "YES".green()
        } else {
            "NO".dimmed()
        }
    );

    println!(
        "\n{}",
        format!(
            "--- Packaged Conversations ({}) ---",
            manifest.conversations.len()
        )
        .bold()
    );
    for (i, conv) in manifest.conversations.iter().enumerate() {
        let title = if conv.title.trim().is_empty() {
            "Untitled Conversation"
        } else {
            &conv.title
        };
        println!(
            "  {}. {} [{}] ({} steps, modified: {})",
            i + 1,
            title.bold(),
            conv.conversation_id.dimmed(),
            conv.step_count,
            conv.last_modified_time
        );
    }
    println!();

    Ok(())
}

fn handle_project_import(
    paths: &AntigravityPaths,
    bundle_path: PathBuf,
    target_workspace: Option<PathBuf>,
    overwrite: bool,
    restore_customizations: bool,
    dry_run: bool,
    restart: bool,
) -> Result<()> {
    let target_ws = target_workspace
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let options = ProjectImportOptions {
        bundle_path: bundle_path.clone(),
        target_workspace: target_ws.clone(),
        overwrite,
        dry_run,
        restore_customizations,
    };

    println!(
        "\n{}",
        "● Importing Project Archive (.agproject)...".bold().cyan()
    );
    if dry_run {
        println!(
            "{}",
            "MODE: DRY-RUN (No files will be modified)".yellow().bold()
        );
    }
    println!("  Bundle File     : {:?}", bundle_path);
    println!("  Target Workspace: {:?}", target_ws);
    println!(
        "  Customizations  : {}",
        if restore_customizations {
            "YES (Restoring skills, rules, MCP)".green()
        } else {
            "NO (Skipped)".yellow()
        }
    );
    println!(
        "  Overwrite       : {}",
        if overwrite {
            "YES".yellow()
        } else {
            "NO (Safe-forking collisions)".green()
        }
    );

    let (count, manifest) = ProjectImporter::import_project_bundle(paths, &options)?;

    println!("\n{}", "--- Import Summary ---".bold());
    println!("  Project Name         : {}", manifest.project_name.bold());
    println!(
        "  Conversations Mapped : {}",
        count.to_string().cyan().bold()
    );
    println!(
        "  Total Artifacts      : {}",
        manifest.total_artifacts_count
    );

    if dry_run {
        println!(
            "\n{}",
            "Dry-run complete. Run without --dry-run to execute import.".green()
        );
    } else {
        println!(
            "\n{}",
            format!(
                "✔ Successfully imported project! Restored {} conversations and workspace context.",
                count
            )
            .bold()
            .green()
        );

        if restart {
            if IdeStorage::is_antigravity_running() {
                println!(
                    "{}",
                    "● Restarting Antigravity to reload projects and language server...".cyan()
                );
                match IdeStorage::restart_antigravity(Some(&options.target_workspace)) {
                    Ok(_) => println!(
                        "{}",
                        "✔ Successfully restarted Antigravity! Changes are now active."
                            .green()
                            .bold()
                    ),
                    Err(e) => {
                        eprintln!("{}", format!("⚠ Failed to restart Antigravity: {}", e).yellow())
                    }
                }
            } else {
                println!(
                    "{}",
                    "Antigravity is not currently running. Projects and sessions will be loaded on next startup."
                        .dimmed()
                );
            }
        } else if IdeStorage::is_antigravity_running() {
            println!(
                "{}",
                "💡 Tip: Antigravity is currently running. Run 'agx restart' (or pass --restart) to view your imported project and conversations immediately."
                    .dimmed()
            );
        }
    }
    println!();

    Ok(())
}

fn handle_restart(workspace: Option<PathBuf>) -> Result<()> {
    let ws = workspace.or_else(|| std::env::current_dir().ok());
    if IdeStorage::is_antigravity_running() {
        println!(
            "{}",
            "● Restarting Antigravity to reload projects and language server...".cyan()
        );
        match IdeStorage::restart_antigravity(ws.as_deref()) {
            Ok(_) => println!(
                "{}",
                "✔ Antigravity restarted successfully! Projects and sessions are now loaded."
                    .green()
                    .bold()
            ),
            Err(e) => eprintln!("{}", format!("⚠ Failed to restart Antigravity: {}", e).yellow()),
        }
    } else {
        println!(
            "{}",
            "● Antigravity is not currently running. Launching...".cyan()
        );
        match IdeStorage::launch_antigravity(ws.as_deref()) {
            Ok(_) => println!(
                "{}",
                "✔ Antigravity launched successfully!".green().bold()
            ),
            Err(e) => eprintln!("{}", format!("⚠ Failed to launch Antigravity: {}", e).yellow()),
        }
    }
    println!();
    Ok(())
}
