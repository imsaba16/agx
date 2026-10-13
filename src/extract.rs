use crate::bundler::open_archive;
use crate::security::{SafePathValidator, TarLimitGuard};
use anyhow::{Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ArchiveEntryMeta {
    pub path: String,
    pub size: u64,
}

pub struct Extractor;

impl Extractor {
    /// Lists all files contained within an archive
    pub fn list_files(archive_path: &Path) -> Result<Vec<ArchiveEntryMeta>> {
        let mut archive = open_archive(archive_path)?;
        let mut entries_meta = Vec::new();

        for entry_res in archive.entries()? {
            let entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");
            let size = entry.header().size()?;

            entries_meta.push(ArchiveEntryMeta {
                path: path_str,
                size,
            });
        }

        Ok(entries_meta)
    }

    /// Read a specific file from an archive into memory matching a pattern (e.g. "plan", "walkthrough", "transcript")
    pub fn read_file_content(archive_path: &Path, pattern: &str) -> Result<(String, Vec<u8>)> {
        let mut archive = open_archive(archive_path)?;
        let pat_lower = pattern.to_lowercase();

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");
            let path_lower = path_str.to_lowercase();

            // Match if exact match or contains pattern or standard alias
            let is_match = path_str == pattern
                || path_lower.contains(&pat_lower)
                || (pattern == "plan" && path_lower.ends_with("implementation_plan.md"))
                || (pattern == "walkthrough" && path_lower.ends_with("walkthrough.md"))
                || (pattern == "transcript" && path_lower.ends_with("transcript.jsonl"))
                || (pattern == "manifest"
                    && (path_str == "manifest.json" || path_str == "project_manifest.json"));

            if is_match {
                let mut content = Vec::new();
                entry.read_to_end(&mut content)?;
                return Ok((path_str, content));
            }
        }

        anyhow::bail!(
            "No file matching '{}' found in archive {:?}",
            pattern,
            archive_path
        );
    }

    /// Surgically extracts matching files to an output destination or current directory
    pub fn extract(
        archive_path: &Path,
        pattern: Option<&str>,
        output_dest: Option<&Path>,
    ) -> Result<Vec<PathBuf>> {
        let mut archive = open_archive(archive_path)?;
        let mut limit_guard = TarLimitGuard::default();
        let target_dir = output_dest
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut extracted_paths = Vec::new();

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            let path_buf = entry.path()?.to_path_buf();
            let path_str = path_buf.to_string_lossy().replace('\\', "/");
            let path_lower = path_str.to_lowercase();

            let matches = if let Some(pat) = pattern {
                let pat_lower = pat.to_lowercase();
                path_str == pat
                    || path_lower.contains(&pat_lower)
                    || (pat == "plan" && path_lower.ends_with("implementation_plan.md"))
                    || (pat == "walkthrough" && path_lower.ends_with("walkthrough.md"))
                    || (pat == "transcript" && path_lower.ends_with("transcript.jsonl"))
                    || (pat == "manifest"
                        && (path_str == "manifest.json" || path_str == "project_manifest.json"))
            } else {
                true
            };

            if !matches {
                continue;
            }

            let entry_len = entry.header().size()?;
            limit_guard.check_and_add(entry_len)?;

            let filename = path_buf.file_name().unwrap_or(path_buf.as_os_str());
            let out_file_path = if target_dir.is_dir() {
                SafePathValidator::validate_and_resolve(&target_dir, Path::new(filename))?
            } else {
                target_dir.clone()
            };

            if let Some(parent) = out_file_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory {:?}", parent))?;
            }

            entry
                .unpack(&out_file_path)
                .with_context(|| format!("Failed to extract {:?}", out_file_path))?;

            extracted_paths.push(out_file_path);
        }

        if extracted_paths.is_empty() {
            if let Some(pat) = pattern {
                anyhow::bail!("No files matching '{}' found in {:?}", pat, archive_path);
            } else {
                anyhow::bail!("No files found to extract in {:?}", archive_path);
            }
        }

        Ok(extracted_paths)
    }
}
