use crate::config::AntigravityPaths;
use crate::models::{BundleManifest, ConversationSummary, ExportOptions};
use crate::remapper::PathRemapper;
use crate::sanitizer::SecretSanitizer;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use rayon::prelude::*;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder, Header};
use walkdir::WalkDir;

pub struct Bundler;

struct ProcessedFile {
    tar_entry_name: String,
    data: Vec<u8>,
}

impl Bundler {
    pub fn export_bundle(
        paths: &AntigravityPaths,
        mut summary: ConversationSummary,
        options: &ExportOptions,
    ) -> Result<BundleManifest> {
        let conv_id = &summary.conversation_id;
        let brain_path = paths.conversation_brain(conv_id);
        let conv_db_path = paths.conversation_db(conv_id);

        let remapper = options.workspace_root.as_ref().map(|root| PathRemapper::for_export(root));
        let sanitizer = if options.sanitize {
            Some(SecretSanitizer::new())
        } else {
            None
        };

        // Extract original workspace paths from summary.workspace_uris
        let original_workspace_paths: Vec<String> = serde_json::from_str(&summary.workspace_uris)
            .unwrap_or_else(|_| vec![summary.workspace_uris.clone()]);

        // Remap summary workspace URIs
        if let Some(ref remap) = remapper {
            summary.workspace_uris = remap.remap_workspace_uris_export(&summary.workspace_uris);
        }

        // Initialize ignore filter
        let filter = crate::filter::IgnoreFilter::new(
            options.compact,
            options.custom_ignore_file.as_deref(),
            options.workspace_root.as_deref(),
        );

        // Collect candidate brain files
        let mut candidate_entries: Vec<PathBuf> = Vec::new();
        let mut artifacts_count = 0;
        let mut ignored_count = 0;

        if brain_path.exists() {
            for entry in WalkDir::new(&brain_path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Ok(rel_path) = entry.path().strip_prefix(&brain_path) {
                        if filter.is_ignored(rel_path) {
                            ignored_count += 1;
                        } else {
                            artifacts_count += 1;
                            candidate_entries.push(entry.path().to_path_buf());
                        }
                    }
                }
            }
        }

        let has_db = conv_db_path.exists();

        let manifest = BundleManifest {
            version: "1.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_os: std::env::consts::OS.to_string(),
            source_username: whoami(),
            conversation: summary,
            sanitized: options.sanitize,
            compact: options.compact,
            original_workspace_paths,
            artifacts_count,
            ignored_count,
            has_db,
        };

        // Parallel processing of brain files across all CPU cores with Rayon
        let remapper_ref = remapper.as_ref();
        let sanitizer_ref = sanitizer.as_ref();
        let brain_path_ref = &brain_path;

        let processed_files: Vec<Result<ProcessedFile>> = candidate_entries
            .into_par_iter()
            .map(|entry_path| {
                let rel_path = entry_path.strip_prefix(brain_path_ref)?;
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                let tar_entry_name = format!("brain/{}", rel_str);

                let is_text = is_text_file(&entry_path);
                let data = if is_text {
                    if let Ok(content) = std::fs::read_to_string(&entry_path) {
                        let mut processed = content;
                        if let Some(remap) = remapper_ref {
                            processed = remap.remap_export(&processed);
                        }
                        if let Some(sanit) = sanitizer_ref {
                            processed = sanit.sanitize(&processed);
                        }
                        processed.into_bytes()
                    } else {
                        std::fs::read(&entry_path)?
                    }
                } else {
                    std::fs::read(&entry_path)?
                };

                Ok(ProcessedFile {
                    tar_entry_name,
                    data,
                })
            })
            .collect();

        // Create zstd compressed archive (level 3 for ultra-fast compression and high ratio)
        let file = File::create(&options.output_path)
            .with_context(|| format!("Failed to create output archive at {:?}", options.output_path))?;
        let zstd_enc = zstd::stream::write::Encoder::new(file, 3)?;
        let mut tar = Builder::new(zstd_enc);

        // 1. Write manifest.json
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        let mut header = Header::new_gnu();
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "manifest.json", &manifest_bytes[..])?;

        // 2. Write conversation.db if exists
        if has_db {
            let mut db_file = File::open(&conv_db_path)?;
            let mut db_bytes = Vec::new();
            db_file.read_to_end(&mut db_bytes)?;
            let mut db_header = Header::new_gnu();
            db_header.set_size(db_bytes.len() as u64);
            db_header.set_mode(0o644);
            db_header.set_cksum();
            tar.append_data(&mut db_header, "conversation.db", &db_bytes[..])?;
        }

        // 3. Write parallel-processed brain files
        for pf_res in processed_files {
            let pf = pf_res?;
            let mut file_header = Header::new_gnu();
            file_header.set_size(pf.data.len() as u64);
            file_header.set_mode(0o644);
            file_header.set_cksum();
            tar.append_data(&mut file_header, pf.tar_entry_name, &pf.data[..])?;
        }

        tar.finish()?;
        let encoder = tar.into_inner()?;
        encoder.finish()?;

        Ok(manifest)
    }

    pub fn inspect_bundle(bundle_path: &Path) -> Result<BundleManifest> {
        let mut archive = open_archive(bundle_path)?;

        for entry_res in archive.entries()? {
            let mut entry = entry_res?;
            if entry.path()?.to_string_lossy() == "manifest.json" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                let manifest: BundleManifest = serde_json::from_str(&content)?;
                return Ok(manifest);
            }
        }

        anyhow::bail!("Bundle does not contain a valid manifest.json");
    }
}

/// Open an archive supporting both Zstandard (.zst) and Gzip (.gz) automatically via magic bytes
pub fn open_archive(path: &Path) -> Result<Archive<Box<dyn Read>>> {
    let mut file = File::open(path)
        .with_context(|| format!("Failed to open bundle at {:?}", path))?;

    let mut magic = [0u8; 4];
    let n = file.read(&mut magic)?;
    file.seek(SeekFrom::Start(0))?;

    let reader: Box<dyn Read> = if n >= 4 && magic == [0x28, 0xb5, 0x2f, 0xfd] {
        // Zstandard archive
        Box::new(zstd::stream::read::Decoder::new(file)?)
    } else if n >= 2 && magic[0] == 0x1f && magic[1] == 0x8b {
        // Gzip archive (backward compatibility)
        Box::new(GzDecoder::new(file))
    } else {
        // Fallback: try zstd, else gzip
        if let Ok(dec) = zstd::stream::read::Decoder::new(file) {
            Box::new(dec)
        } else {
            let f2 = File::open(path)?;
            Box::new(GzDecoder::new(f2))
        }
    };

    Ok(Archive::new(reader))
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
        "md" | "json" | "jsonl" | "txt" | "py" | "rs" | "ts" | "js" | "sh" | "html" | "css" | "yml" | "yaml" | "toml" | "xml"
    )
}
