use crate::config::AntigravityPaths;
use crate::db::Database;
use crate::models::{BundleManifest, ImportOptions};
use crate::remapper::PathRemapper;
use anyhow::Result;
use std::fs;
use std::io::Read;
use std::path::Path;

pub struct Importer;

impl Importer {
    pub fn import_bundle(
        paths: &AntigravityPaths,
        options: &ImportOptions,
    ) -> Result<(String, BundleManifest)> {
        // Inspect manifest from bundle
        let manifest = crate::bundler::Bundler::inspect_bundle(&options.bundle_path)?;
        let original_id = manifest.conversation.conversation_id.clone();

        let mut final_id = original_id.clone();
        let already_exists = paths.conversation_brain(&original_id).exists()
            || paths.conversation_db(&original_id).exists();

        if already_exists && !options.overwrite {
            final_id = uuid::Uuid::new_v4().to_string();
        }

        if options.dry_run {
            return Ok((final_id, manifest));
        }

        // Ensure directories exist
        fs::create_dir_all(paths.conversations_dir())?;
        fs::create_dir_all(paths.brain_dir())?;

        let target_brain_dir = paths.conversation_brain(&final_id);
        fs::create_dir_all(&target_brain_dir)?;

        let mut updated_summary = manifest.conversation.clone();
        updated_summary.conversation_id = final_id.clone();

        if already_exists && !options.overwrite {
            updated_summary.title = format!("{} (Imported)", updated_summary.title);
        }

        if let Some(ref target_ws) = options.target_workspace {
            updated_summary.workspace_uris =
                PathRemapper::remap_workspace_uris_import(&updated_summary.workspace_uris, target_ws);
        }

        // Re-read archive to extract files using universal loader
        let mut archive = crate::bundler::open_archive(&options.bundle_path)?;

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().to_string();

            if path_str == "manifest.json" {
                continue;
            } else if path_str == "conversation.db" {
                let dest_db_path = paths.conversation_db(&final_id);
                entry.unpack(&dest_db_path)?;
            } else if path_str.starts_with("brain/") {
                let rel = path_buf.strip_prefix("brain")?;
                let dest_file_path = target_brain_dir.join(rel);

                if let Some(parent) = dest_file_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                if is_text_path(&path_buf) && options.target_workspace.is_some() {
                    let mut text_buf = String::new();
                    if entry.read_to_string(&mut text_buf).is_ok() {
                        let target_ws = options.target_workspace.as_ref().unwrap();
                        let remapped = PathRemapper::remap_import(&text_buf, target_ws);
                        fs::write(&dest_file_path, remapped)?;
                    } else {
                        entry.unpack(&dest_file_path)?;
                    }
                } else {
                    entry.unpack(&dest_file_path)?;
                }
            }
        }

        // Update database
        if paths.summaries_db().exists() {
            let db = Database::new(&paths.summaries_db());
            db.upsert_conversation(&updated_summary)?;
        }

        Ok((final_id, manifest))
    }
}

fn is_text_path(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext.to_lowercase().as_str(),
        "md" | "json" | "jsonl" | "txt" | "py" | "rs" | "ts" | "js" | "sh" | "html" | "css" | "yml" | "yaml" | "toml" | "xml"
    )
}
