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

        // 1. Verify project bundle integrity and security limits
        let verification = ProjectBundler::verify_project_bundle(&options.bundle_path)?;
        if !verification.is_valid {
            anyhow::bail!(
                "Security check failed: Project bundle integrity check failed. Errors: {:?}, Mismatches: {:?}",
                verification.errors,
                verification.mismatched_files
            );
        }

        // 2. Setup atomic staging directory
        let parent_dir = paths.base_dir.parent().unwrap_or(&paths.base_dir);
        let staging_dir = tempfile::tempdir_in(parent_dir)?;
        let staging_sessions = staging_dir.path().join("sessions");
        let staging_custom = staging_dir.path().join("customizations");
        fs::create_dir_all(&staging_sessions)?;
        fs::create_dir_all(&staging_custom)?;

        let mut limit_guard = crate::security::TarLimitGuard::default();

        let mut archive = crate::bundler::open_archive(&options.bundle_path)?;

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");

            let entry_len = entry.header().size()?;
            limit_guard.check_and_add(entry_len)?;

            if path_str == "project_manifest.json" {
                continue;
            } else if path_str.starts_with("customizations/") {
                if options.restore_customizations {
                    let rel = path_buf.strip_prefix("customizations")?;
                    let staged_dest = crate::security::SafePathValidator::validate_and_resolve(
                        &staging_custom,
                        rel,
                    )?;

                    if let Some(parent) = staged_dest.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if is_text_file(&staged_dest) {
                        let mut text = String::new();
                        if entry.read_to_string(&mut text).is_ok() {
                            let remapped = PathRemapper::remap_import(&text, target_ws);
                            fs::write(&staged_dest, remapped)?;
                        } else {
                            entry.unpack(&staged_dest)?;
                        }
                    } else {
                        entry.unpack(&staged_dest)?;
                    }
                }
            } else if path_str.starts_with("sessions/") {
                // Path format: sessions/<orig_id>/conversation.db OR sessions/<orig_id>/brain/<rel>
                let parts: Vec<&str> = path_str.splitn(4, '/').collect();
                if parts.len() >= 3 {
                    let orig_id = parts[1];
                    let final_id = id_map
                        .get(orig_id)
                        .cloned()
                        .unwrap_or_else(|| orig_id.to_string());

                    if parts[2] == "conversation.db" {
                        let staged_db = staging_sessions.join(&final_id).join("conversation.db");
                        if let Some(parent) = staged_db.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        entry.unpack(&staged_db)?;
                    } else if parts[2] == "brain" && parts.len() == 4 {
                        let brain_rel = Path::new(parts[3]);
                        let staged_brain_dir = staging_sessions.join(&final_id).join("brain");
                        let staged_dest = crate::security::SafePathValidator::validate_and_resolve(
                            &staged_brain_dir,
                            brain_rel,
                        )?;

                        if let Some(parent) = staged_dest.parent() {
                            fs::create_dir_all(parent)?;
                        }

                        if is_text_file(&staged_dest) {
                            let mut text = String::new();
                            if entry.read_to_string(&mut text).is_ok() {
                                let remapped = PathRemapper::remap_import(&text, target_ws);
                                fs::write(&staged_dest, remapped)?;
                            } else {
                                entry.unpack(&staged_dest)?;
                            }
                        } else {
                            entry.unpack(&staged_dest)?;
                        }
                    }
                }
            }
        }

        // 3. Atomically commit staged files into final destinations
        fs::create_dir_all(paths.conversations_dir())?;
        fs::create_dir_all(paths.brain_dir())?;

        // Commit sessions
        for conv in &manifest.conversations {
            let orig_id = &conv.conversation_id;
            let final_id = id_map
                .get(orig_id)
                .cloned()
                .unwrap_or_else(|| orig_id.clone());

            let staged_conv_dir = staging_sessions.join(&final_id);
            let staged_db = staged_conv_dir.join("conversation.db");
            if staged_db.exists() {
                let dest_db = paths.conversation_db(&final_id);
                if dest_db.exists() {
                    fs::remove_file(&dest_db)?;
                }
                fs::rename(&staged_db, &dest_db)
                    .or_else(|_| fs::copy(&staged_db, &dest_db).map(|_| ()))?;
            }

            let staged_brain = staged_conv_dir.join("brain");
            if staged_brain.exists() {
                let dest_brain = paths.conversation_brain(&final_id);
                if dest_brain.exists() {
                    fs::remove_dir_all(&dest_brain)?;
                }
                fs::rename(&staged_brain, &dest_brain)
                    .or_else(|_| copy_dir_recursive(&staged_brain, &dest_brain))?;
            }
        }

        // Commit customizations
        if options.restore_customizations && staging_custom.exists() {
            for entry in walkdir::WalkDir::new(&staging_custom)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    if let Ok(rel) = entry.path().strip_prefix(&staging_custom) {
                        let dest = target_ws.join(rel);
                        if dest.exists() && !options.overwrite {
                            continue;
                        }
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::copy(entry.path(), &dest)?;
                    }
                }
            }
        }

        // 4. Upsert conversations in summaries database
        if paths.summaries_db().exists() {
            let db = Database::new(&paths.summaries_db());
            for conv in &manifest.conversations {
                let orig_id = &conv.conversation_id;
                let final_id = id_map
                    .get(orig_id)
                    .cloned()
                    .unwrap_or_else(|| orig_id.clone());

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
        "md" | "json"
            | "jsonl"
            | "txt"
            | "py"
            | "rs"
            | "ts"
            | "js"
            | "sh"
            | "html"
            | "css"
            | "yml"
            | "yaml"
            | "toml"
            | "xml"
    )
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
