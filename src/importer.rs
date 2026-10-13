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

        // 1. Verify bundle integrity & security before modifying anything
        let verification = crate::bundler::Bundler::verify_bundle(&options.bundle_path)?;
        if !verification.is_valid {
            anyhow::bail!(
                "Security check failed: Bundle integrity check failed. Errors: {:?}, Mismatches: {:?}",
                verification.errors,
                verification.mismatched_files
            );
        }

        // 2. Setup atomic staging directory
        let parent_dir = paths.base_dir.parent().unwrap_or(&paths.base_dir);
        let staging_dir = tempfile::tempdir_in(parent_dir)?;
        let staging_brain = staging_dir.path().join("brain");
        let staging_conv_db = staging_dir.path().join("conversation.db");
        fs::create_dir_all(&staging_brain)?;

        let mut limit_guard = crate::security::TarLimitGuard::default();

        // 3. Extract and remap into atomic staging directory
        let mut archive = crate::bundler::open_archive(&options.bundle_path)?;
        let mut has_conv_db = false;

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");

            let entry_len = entry.header().size()?;
            limit_guard.check_and_add(entry_len)?;

            if path_str == "manifest.json" {
                continue;
            } else if path_str == "conversation.db" {
                entry.unpack(&staging_conv_db)?;
                has_conv_db = true;
            } else if path_str.starts_with("brain/") {
                let rel = path_buf.strip_prefix("brain")?;
                let dest_file_path = crate::security::SafePathValidator::validate_and_resolve(&staging_brain, rel)?;

                if let Some(parent) = dest_file_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                if is_text_path(&path_buf) {
                    if let Some(ref target_ws) = options.target_workspace {
                        let mut text_buf = String::new();
                        if entry.read_to_string(&mut text_buf).is_ok() {
                            let remapped = PathRemapper::remap_import(&text_buf, target_ws);
                            fs::write(&dest_file_path, remapped)?;
                        } else {
                            entry.unpack(&dest_file_path)?;
                        }
                    } else {
                        entry.unpack(&dest_file_path)?;
                    }
                } else {
                    entry.unpack(&dest_file_path)?;
                }
            }
        }

        // 4. Atomically commit staged files into final destinations
        fs::create_dir_all(paths.conversations_dir())?;
        fs::create_dir_all(paths.brain_dir())?;

        let target_brain_dir = paths.conversation_brain(&final_id);
        if target_brain_dir.exists() {
            fs::remove_dir_all(&target_brain_dir)?;
        }
        // Move staging brain directory to target
        fs::rename(&staging_brain, &target_brain_dir).or_else(|_| {
            // Fallback for cross-filesystem moves
            copy_dir_recursive(&staging_brain, &target_brain_dir)
        })?;

        if has_conv_db {
            let dest_db_path = paths.conversation_db(&final_id);
            if dest_db_path.exists() {
                fs::remove_file(&dest_db_path)?;
            }
            fs::rename(&staging_conv_db, &dest_db_path).or_else(|_| {
                fs::copy(&staging_conv_db, &dest_db_path).map(|_| ())
            })?;
        }

        // 5. Update database
        if paths.summaries_db().exists() {
            let mut updated_summary = manifest.conversation.clone();
            updated_summary.conversation_id = final_id.clone();
            if already_exists && !options.overwrite {
                updated_summary.title = format!("{} (Imported)", updated_summary.title);
            }
            if let Some(ref target_ws) = options.target_workspace {
                updated_summary.workspace_uris =
                    PathRemapper::remap_workspace_uris_import(&updated_summary.workspace_uris, target_ws);
            }

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

