use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::{Path, PathBuf};

use agx::bundler::Bundler;
use agx::config::AntigravityPaths;
use agx::db::Database;
use agx::importer::Importer;
use agx::models::{ExportOptions, ImportOptions};

#[derive(Parser, Debug)]
#[command(name = "agx")]
#[command(author = "Antigravity Pair Programmer")]
#[command(version = "0.1.0")]
#[command(about = "agx: Portable backup, sync, and export tool for Google Antigravity brains, conversations, and artifacts", long_about = None)]
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
    },

    /// Export a conversation and its project brain into a portable .agbundle archive
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
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AntigravityPaths::discover(cli.data_dir)?;

    match cli.command {
        Some(Commands::List { workspace, limit }) => {
            handle_list(&paths, workspace, limit)?;
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
        Some(Commands::Info { bundle }) => {
            handle_info(&bundle)?;
        }
        Some(Commands::Import {
            bundle,
            target_workspace,
            overwrite,
            dry_run,
        }) => {
            handle_import(&paths, bundle, target_workspace, overwrite, dry_run)?;
        }
        None => {
            run_interactive_dashboard(&paths)?;
        }
    }

    Ok(())
}

fn run_interactive_dashboard(paths: &AntigravityPaths) -> Result<()> {
    println!("\n{}", "⚡ agx: Antigravity Context & Brain eXchange".bold().cyan());
    println!("{}", "================================================".dimmed());

    loop {
        let options = vec![
            "📋 1. List conversations",
            "📦 2. Export a conversation (Interactive Picker)",
            "⚡ 3. Export a conversation (Compact Mode)",
            "🔍 4. Inspect an .agbundle file",
            "📥 5. Import an .agbundle file",
            "🚪 6. Exit",
        ];

        let selection = match inquire::Select::new("What would you like to do?", options).prompt() {
            Ok(s) => s,
            Err(_) => break,
        };

        if selection.starts_with("📋 1") {
            let _ = handle_list(paths, None, 20);
        } else if selection.starts_with("📦 2") {
            let _ = handle_export(paths, None, None, None, true, false, true, None);
        } else if selection.starts_with("⚡ 3") {
            let _ = handle_export(paths, None, None, None, true, true, true, None);
        } else if selection.starts_with("🔍 4") {
            let path_str = match inquire::Text::new("Enter path to .agbundle:").prompt() {
                Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
                _ => continue,
            };
            let _ = handle_info(Path::new(&path_str));
        } else if selection.starts_with("📥 5") {
            let bundle_path_str = match inquire::Text::new("Enter path to .agbundle to import:").prompt() {
                Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
                _ => continue,
            };
            let ws_path_str = match inquire::Text::new("Target workspace path (press Enter for current folder):").prompt() {
                Ok(s) => s.trim().to_string(),
                _ => String::new(),
            };
            let target_ws = if ws_path_str.is_empty() {
                None
            } else {
                Some(PathBuf::from(ws_path_str))
            };
            let _ = handle_import(paths, PathBuf::from(bundle_path_str), target_ws, false, false);
        } else {
            break;
        }

        println!();
    }

    println!("\nPress Enter to exit...");
    let mut dummy = String::new();
    let _ = std::io::stdin().read_line(&mut dummy);
    Ok(())
}

fn handle_list(paths: &AntigravityPaths, workspace_filter: Option<String>, limit: usize) -> Result<()> {
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
                c.workspace_uris.to_lowercase().contains(&filter.to_lowercase())
            } else {
                true
            }
        })
        .take(limit)
        .collect();

    if filtered.is_empty() {
        println!("{}", "No matching conversations found.".yellow());
        return Ok(());
    }

    println!("\n{}", "=== Antigravity Conversations ===".bold().cyan());
    for conv in &filtered {
        let brain_exists = paths.conversation_brain(&conv.conversation_id).exists();
        let db_exists = paths.conversation_db(&conv.conversation_id).exists();

        let brain_badge = if brain_exists {
            "Brain: YES".green()
        } else {
            "Brain: NO".dimmed()
        };

        let db_badge = if db_exists {
            "DB: YES".green()
        } else {
            "DB: NO".dimmed()
        };

        println!(
            "\n• {} [{}]",
            conv.title.bold().white(),
            conv.conversation_id.dimmed()
        );
        println!("  Last Modified : {}", conv.last_modified_time.cyan());
        println!("  Workspaces    : {}", conv.workspace_uris);
        println!(
            "  Steps         : {} | {} | {}",
            conv.step_count.to_string().yellow(),
            brain_badge,
            db_badge
        );
    }
    println!("\nTotal displayed: {}\n", filtered.len());

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

            let display = format!("[{}] {} ({} steps)", date, c.title, c.step_count);

            Item {
                summary: c,
                display,
            }
        })
        .collect();

    let ans = inquire::Select::new("Select conversation to export:", items)
        .with_page_size(15)
        .with_help_message("Use ↑/↓ arrows to navigate, type to filter, Esc to cancel")
        .prompt();

    match ans {
        Ok(selected) => Ok(selected.summary.clone()),
        Err(inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted) => {
            println!("{}", "Export cancelled by user.".yellow());
            std::process::exit(0);
        }
        Err(e) => Err(anyhow::anyhow!("Interactive selection failed: {}", e)),
    }
}

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
                format!("Auto-detected active conversation for current directory: {}", conv.title).green()
            );
            (conv.conversation_id.clone(), conv)
        } else {
            let all = db.list_conversations()?;
            println!(
                "{}",
                "No active conversation matched current directory. Opening interactive picker...".yellow()
            );
            let selected = select_conversation_interactively(&all)?;
            (selected.conversation_id.clone(), selected)
        }
    };

    let ws_root = workspace_root
        .or_else(|| std::env::current_dir().ok());

    let final_output = output_path.unwrap_or_else(|| {
        let sanitized_title: String = summary
            .title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let suffix = if compact { "_compact" } else { "" };
        PathBuf::from(format!("{}{}_{}.agbundle", sanitized_title, suffix, &conv_id[..8]))
    });

    let options = ExportOptions {
        conversation_id: conv_id.clone(),
        output_path: final_output.clone(),
        sanitize,
        compact,
        workspace_root: ws_root.clone(),
        custom_ignore_file: ignore_file,
    };

    println!("\n{}", "📦 Packaging Antigravity Brain Bundle...".bold().cyan());
    println!("  Title       : {}", summary.title.bold());
    println!("  ID          : {}", conv_id);
    println!("  Sanitize    : {}", if sanitize { "YES (Masking secrets)".green() } else { "NO".yellow() });
    println!("  Compact Mode: {}", if compact { "YES (Skipping raw step logs & heavy media)".green() } else { "NO (Full fidelity)".dimmed() });
    if let Some(ref ws) = ws_root {
        println!("  Remap Root  : {:?}", ws);
    }
    println!("  Output File : {:?}", final_output);

    let manifest = Bundler::export_bundle(paths, summary, &options)?;

    println!(
        "\n{}",
        format!(
            "✅ Successfully exported bundle! Packaged {} brain files ({} files excluded by .agignore/compact filter).",
            manifest.artifacts_count,
            manifest.ignored_count
        )
        .bold()
        .green()
    );
    println!("Archive created at: {:?}\n", final_output);

    Ok(())
}

fn handle_info(bundle_path: &Path) -> Result<()> {
    let manifest = Bundler::inspect_bundle(bundle_path)?;

    println!("\n{}", "=== Antigravity Bundle Information ===".bold().cyan());
    println!("  Bundle Version : {}", manifest.version.yellow());
    println!("  Created At     : {}", manifest.created_at.cyan());
    println!("  Source Host    : {} ({})", manifest.source_username, manifest.source_os);
    println!("  Sanitized      : {}", if manifest.sanitized { "YES".green() } else { "NO".yellow() });
    println!("  Compact Mode   : {}", if manifest.compact { "YES".green() } else { "NO".dimmed() });
    println!("  Artifacts Count: {}", manifest.artifacts_count.to_string().bold());
    println!("  Excluded Count : {}", manifest.ignored_count.to_string().yellow());
    println!("  Includes DB    : {}", if manifest.has_db { "YES".green() } else { "NO".red() });
    println!("\n{}", "--- Conversation Details ---".bold());
    println!("  Title          : {}", manifest.conversation.title.bold().white());
    println!("  Original ID    : {}", manifest.conversation.conversation_id);
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
) -> Result<()> {
    let options = ImportOptions {
        bundle_path: bundle_path.clone(),
        target_workspace: target_workspace.or_else(|| std::env::current_dir().ok()),
        overwrite,
        dry_run,
    };

    println!("\n{}", "📥 Importing Antigravity Bundle...".bold().cyan());
    if dry_run {
        println!("{}", "MODE: DRY-RUN (No files will be modified)".yellow().bold());
    }

    let (imported_id, manifest) = Importer::import_bundle(paths, &options)?;

    println!("  Title           : {}", manifest.conversation.title.bold());
    println!("  Original ID     : {}", manifest.conversation.conversation_id);
    println!("  Assigned ID     : {}", imported_id.cyan());
    if let Some(ref target_ws) = options.target_workspace {
        println!("  Bound Workspace : {:?}", target_ws);
    }
    println!("  Artifacts Count : {}", manifest.artifacts_count);

    if dry_run {
        println!("\n{}", "Dry-run complete. Run without --dry-run to execute import.".green());
    } else {
        println!(
            "\n{}",
            format!(
                "✅ Successfully imported into Antigravity! Ready to use in chat or IDE."
            )
            .bold()
            .green()
        );
    }
    println!();

    Ok(())
}
