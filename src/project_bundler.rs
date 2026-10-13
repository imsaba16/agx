use crate::config::AntigravityPaths;
use crate::db::Database;
use crate::filter::IgnoreFilter;
use crate::models::{
    ConversationSummary, ProjectCustomizations, ProjectExportOptions, ProjectManifest,
};
use crate::remapper::PathRemapper;
use crate::sanitizer::SecretSanitizer;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::{Builder, Header};
use walkdir::WalkDir;

pub struct ProjectBundler;

struct ArchiveEntry {
    tar_path: String,
    data: Vec<u8>,
    sha256: String,
}

impl ProjectBundler {
    pub fn export_project_bundle(
        paths: &AntigravityPaths,
        options: &ProjectExportOptions,
    ) -> Result<ProjectManifest> {
        let ws_root = &options.workspace_root;
        let project_name = ws_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        let db = Database::new(&paths.summaries_db());
        let all_convs = db.find_conversations_for_workspace(ws_root)?;

        let selected_convs: Vec<ConversationSummary> = if let Some(ref ids) = options.session_ids {
            all_convs
                .into_iter()
                .filter(|c| ids.contains(&c.conversation_id))
                .collect()
        } else {
            all_convs
        };

        if selected_convs.is_empty() {
            anyhow::bail!(
                "No matching Antigravity conversations found for workspace {:?}",
                ws_root
            );
        }

        let spinner = if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            let s = indicatif::ProgressBar::new_spinner();
            s.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.cyan} {msg}")
                    .unwrap_or_else(|_| indicatif::ProgressStyle::default_spinner()),
            );
            s.set_message(
                "Parallel-processing sessions, brain artifacts, and project customizations...",
            );
            s.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(s)
        } else {
            None
        };

        let remapper = PathRemapper::for_export(ws_root);
        let sanitizer = if options.sanitize {
            Some(SecretSanitizer::new())
        } else {
            None
        };
        let filter = IgnoreFilter::new(
            options.compact,
            options.custom_ignore_file.as_deref(),
            Some(ws_root),
        );

        // Check project customizations
        let mut customizations = ProjectCustomizations {
            has_skills: false,
            skills_count: 0,
            has_rules: false,
            has_mcp: false,
            has_hooks: false,
        };

        let agents_dir = ws_root.join(".agents");
        let skills_dir = agents_dir.join("skills");
        if skills_dir.exists() {
            customizations.has_skills = true;
            if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                customizations.skills_count = entries.filter_map(|e| e.ok()).count();
            }
        }
        if ws_root.join("GEMINI.md").exists()
            || ws_root.join("AGENTS.md").exists()
            || agents_dir.join("rules").exists()
        {
            customizations.has_rules = true;
        }
        if agents_dir.join("mcp_config.json").exists() || ws_root.join("mcp_config.json").exists() {
            customizations.has_mcp = true;
        }
        if agents_dir.join("hooks.json").exists() {
            customizations.has_hooks = true;
        }

        // Remap conversations metadata
        let mut remapped_convs = selected_convs.clone();
        for c in &mut remapped_convs {
            c.workspace_uris = remapper.remap_workspace_uris_export(&c.workspace_uris);
        }

        // Collect all file processing jobs
        let mut total_artifacts_count = 0;
        let mut total_ignored_count = 0;
        let mut file_jobs: Vec<(PathBuf, String, bool)> = Vec::new(); // (source_path, tar_path, is_text)

        // 1. Collect conversation DBs and brain artifacts
        for conv in &selected_convs {
            let conv_id = &conv.conversation_id;
            let db_path = paths.conversation_db(conv_id);
            if db_path.exists() {
                file_jobs.push((
                    db_path,
                    format!("sessions/{}/conversation.db", conv_id),
                    false,
                ));
            }

            let brain_path = paths.conversation_brain(conv_id);
            if brain_path.exists() {
                for entry in WalkDir::new(&brain_path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        if let Ok(rel) = entry.path().strip_prefix(&brain_path) {
                            if filter.is_ignored(rel) {
                                total_ignored_count += 1;
                            } else {
                                total_artifacts_count += 1;
                                let rel_str = rel.to_string_lossy().replace('\\', "/");
                                let tar_path = format!("sessions/{}/brain/{}", conv_id, rel_str);
                                let is_text = is_text_file(entry.path());
                                file_jobs.push((entry.path().to_path_buf(), tar_path, is_text));
                            }
                        }
                    }
                }
            }
        }

        // 2. Collect project customizations if requested
        if options.include_customizations {
            let custom_roots = [
                ws_root.join(".agents"),
                ws_root.join("GEMINI.md"),
                ws_root.join("AGENTS.md"),
            ];

            for root in custom_roots {
                if root.is_file() {
                    let rel = root.strip_prefix(ws_root)?;
                    let tar_path = format!(
                        "customizations/{}",
                        rel.to_string_lossy().replace('\\', "/")
                    );
                    file_jobs.push((root.clone(), tar_path, true));
                } else if root.is_dir() {
                    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                        if entry.file_type().is_file() {
                            if let Ok(rel) = entry.path().strip_prefix(ws_root) {
                                if !filter.is_ignored(rel) {
                                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                                    let tar_path = format!("customizations/{}", rel_str);
                                    let is_text = is_text_file(entry.path());
                                    file_jobs.push((entry.path().to_path_buf(), tar_path, is_text));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Parallel file processing via Rayon
        let remapper_ref = &remapper;
        let sanitizer_ref = sanitizer.as_ref();

        let processed_entries: Vec<Result<ArchiveEntry>> = file_jobs
            .into_par_iter()
            .map(|(src_path, tar_path, is_text)| {
                let data = if is_text {
                    if let Ok(content) = std::fs::read_to_string(&src_path) {
                        let mut processed = remapper_ref.remap_export(&content);
                        if let Some(sanit) = sanitizer_ref {
                            processed = sanit.sanitize(&processed);
                        }
                        processed.into_bytes()
                    } else {
                        std::fs::read(&src_path)?
                    }
                } else {
                    std::fs::read(&src_path)?
                };

                let sha256 = crate::security::compute_sha256(&data);

                Ok(ArchiveEntry {
                    tar_path,
                    data,
                    sha256,
                })
            })
            .collect();

        let mut checksums = std::collections::HashMap::new();
        let mut verified_entries = Vec::new();
        for entry_res in processed_entries {
            let entry = entry_res?;
            checksums.insert(entry.tar_path.clone(), entry.sha256.clone());
            verified_entries.push(entry);
        }

        let cleaned_root = crate::remapper::clean_path(ws_root);
        let manifest = ProjectManifest {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_os: std::env::consts::OS.to_string(),
            source_username: whoami(),
            project_name,
            original_workspace_path: cleaned_root.to_string_lossy().to_string(),
            original_workspace_uri: crate::remapper::to_file_uri(&cleaned_root),
            conversations: remapped_convs,
            customizations,
            sanitized: options.sanitize,
            compact: options.compact,
            total_artifacts_count,
            total_ignored_count,
            checksums,
        };

        // Write Zstandard archive
        let file = File::create(&options.output_path).with_context(|| {
            format!(
                "Failed to create output archive at {:?}",
                options.output_path
            )
        })?;
        let zstd_enc = zstd::stream::write::Encoder::new(file, 3)?;
        let mut tar = Builder::new(zstd_enc);

        // 1. Write project_manifest.json
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        let mut header = Header::new_gnu();
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "project_manifest.json", &manifest_bytes[..])?;

        // 2. Write all processed entries
        for entry in verified_entries {
            let mut file_header = Header::new_gnu();
            file_header.set_size(entry.data.len() as u64);
            file_header.set_mode(0o644);
            file_header.set_cksum();
            tar.append_data(&mut file_header, entry.tar_path, &entry.data[..])?;
        }

        tar.finish()?;
        let encoder = tar.into_inner()?;
        encoder.finish()?;

        if let Some(s) = spinner {
            s.finish_and_clear();
        }

        Ok(manifest)
    }

    pub fn verify_project_bundle(
        bundle_path: &Path,
    ) -> Result<crate::security::VerificationReport> {
        let manifest = Self::inspect_project_bundle(bundle_path)?;
        let mut archive = crate::bundler::open_archive(bundle_path)?;

        let mut limit_guard = crate::security::TarLimitGuard::default();
        let mut checked = 0;
        let mut matched = 0;
        let mut mismatched = Vec::new();
        let mut errors = Vec::new();
        let mut seen_entries = std::collections::HashSet::new();

        for entry_res in archive.entries()? {
            let mut entry = match entry_res {
                Ok(e) => e,
                Err(err) => {
                    errors.push(format!("Archive entry read error: {}", err));
                    continue;
                }
            };

            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");

            if let Err(e) =
                crate::security::SafePathValidator::validate_and_resolve(Path::new("."), &path_buf)
            {
                errors.push(format!(
                    "Path traversal vulnerability detected in entry '{}': {}",
                    path_str, e
                ));
            }

            let entry_len = entry.header().size()?;
            if let Err(e) = limit_guard.check_and_add(entry_len) {
                errors.push(format!(
                    "Resource limit breached in entry '{}': {}",
                    path_str, e
                ));
            }

            if path_str == "project_manifest.json" {
                continue;
            }

            seen_entries.insert(path_str.clone());
            checked += 1;

            if let Some(expected_sha) = manifest.checksums.get(&path_str) {
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                let actual_sha = crate::security::compute_sha256(&data);

                if actual_sha == *expected_sha {
                    matched += 1;
                } else {
                    mismatched.push(format!(
                        "{} (expected: {}, actual: {})",
                        path_str, expected_sha, actual_sha
                    ));
                }
            }
        }

        let mut missing = Vec::new();
        for expected_path in manifest.checksums.keys() {
            if !seen_entries.contains(expected_path) {
                missing.push(expected_path.clone());
            }
        }

        let is_valid = mismatched.is_empty() && missing.is_empty() && errors.is_empty();

        Ok(crate::security::VerificationReport {
            is_valid,
            total_files_checked: checked,
            matched_checksums: matched,
            mismatched_files: mismatched,
            missing_files: missing,
            errors,
        })
    }

    pub fn inspect_project_bundle(bundle_path: &Path) -> Result<ProjectManifest> {
        let mut archive = crate::bundler::open_archive(bundle_path)?;

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            if entry.path()?.to_string_lossy() == "project_manifest.json" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                let manifest: ProjectManifest = serde_json::from_str(&content)?;
                return Ok(manifest);
            }
        }

        anyhow::bail!("Bundle does not contain a valid project_manifest.json");
    }
}

fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
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
