use crate::config::AntigravityPaths;
use crate::db::Database;
use crate::models::{ProjectImportOptions, ProjectManifest};
use crate::project_bundler::ProjectBundler;
use crate::remapper::PathRemapper;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

pub struct ProjectImporter;

impl ProjectImporter {
    pub fn import_project_bundle(
        paths: &AntigravityPaths,
        options: &ProjectImportOptions,
    ) -> Result<(usize, ProjectManifest)> {
        let manifest = ProjectBundler::inspect_project_bundle(&options.bundle_path)?;
        let target_ws = &options.target_workspace;

        // Build mapping for conversation IDs (handle safe-forking)
        let mut id_map: HashMap<String, String> = HashMap::new();
        for conv in &manifest.conversations {
            let orig_id = &conv.conversation_id;
            let exists = paths.conversation_brain(orig_id).exists()
                || paths.conversation_db(orig_id).exists();

            let final_id = if exists && !options.overwrite {
                uuid::Uuid::new_v4().to_string()
            } else {
                orig_id.clone()
            };
            id_map.insert(orig_id.clone(), final_id);
        }

        if options.dry_run {
            return Ok((manifest.conversations.len(), manifest));
        }

        // Ensure directories exist
        fs::create_dir_all(paths.conversations_dir())?;
        fs::create_dir_all(paths.brain_dir())?;

        let mut archive = crate::bundler::open_archive(&options.bundle_path)?;

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");

            if path_str == "project_manifest.json" {
                continue;
            } else if path_str.starts_with("customizations/") {
                if options.restore_customizations {
                    let rel = path_str.strip_prefix("customizations/").unwrap();
                    let dest = target_ws.join(rel);

                    if dest.exists() && !options.overwrite {
                        continue;
                    }

                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if is_text_file(&dest) {
                        let mut text = String::new();
                        if entry.read_to_string(&mut text).is_ok() {
                            let remapped = PathRemapper::remap_import(&text, target_ws);
                            fs::write(&dest, remapped)?;
                        } else {
                            entry.unpack(&dest)?;
                        }
                    } else {
                        entry.unpack(&dest)?;
                    }
                }
            } else if path_str.starts_with("sessions/") {
                // Path format: sessions/<orig_id>/conversation.db OR sessions/<orig_id>/brain/<rel>
                let parts: Vec<&str> = path_str.splitn(4, '/').collect();
                if parts.len() >= 3 {
                    let orig_id = parts[1];
                    let final_id = id_map.get(orig_id).cloned().unwrap_or_else(|| orig_id.to_string());

                    if parts[2] == "conversation.db" {
                        let dest = paths.conversation_db(&final_id);
                        entry.unpack(&dest)?;
                    } else if parts[2] == "brain" && parts.len() == 4 {
                        let brain_rel = parts[3];
                        let dest = paths.conversation_brain(&final_id).join(brain_rel);

                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent)?;
                        }

                        if is_text_file(&dest) {
                            let mut text = String::new();
                            if entry.read_to_string(&mut text).is_ok() {
                                let remapped = PathRemapper::remap_import(&text, target_ws);
                                fs::write(&dest, remapped)?;
                            } else {
                                entry.unpack(&dest)?;
                            }
                        } else {
                            entry.unpack(&dest)?;
                        }
                    }
                }
            }
        }

        // Upsert conversations in summaries database
        if paths.summaries_db().exists() {
            let db = Database::new(&paths.summaries_db());
            for conv in &manifest.conversations {
                let orig_id = &conv.conversation_id;
                let final_id = id_map.get(orig_id).cloned().unwrap_or_else(|| orig_id.clone());

                let mut updated = conv.clone();
                updated.conversation_id = final_id.clone();
                if final_id != *orig_id {
                    updated.title = format!("{} (Imported)", updated.title);
                }
                updated.workspace_uris =
                    PathRemapper::remap_workspace_uris_import(&updated.workspace_uris, target_ws);

                db.upsert_conversation(&updated)?;
            }
        }

        Ok((manifest.conversations.len(), manifest))
    }
}

fn is_text_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext.to_lowercase().as_str(),
        "md" | "json" | "jsonl" | "txt" | "py" | "rs" | "ts" | "js" | "sh" | "html" | "css" | "yml" | "yaml" | "toml" | "xml"
    )
}
