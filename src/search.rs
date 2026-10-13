use crate::bundler::open_archive;
use crate::config::AntigravityPaths;
use crate::db::Database;
use crate::ui::find_local_archives;
use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub location: String,
    pub inner_file: String,
    pub line_number: usize,
    pub line_content: String,
}

pub struct SearchEngine;

impl SearchEngine {
    /// Search within archives in a directory without uncompressing them to disk
    pub fn search_archives(
        query: &str,
        target_dir: &Path,
        case_sensitive: bool,
    ) -> Result<Vec<SearchHit>> {
        let archives = find_local_archives(target_dir);
        let mut all_hits = Vec::new();

        let q_pattern = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for archive_path in archives {
            let archive_name = archive_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "archive".to_string());

            if let Ok(mut archive) = open_archive(&archive_path) {
                if let Ok(entries) = archive.entries() {
                    for entry_res in entries.flatten() {
                        let path_buf = match entry_res.path() {
                            Ok(p) => p.to_path_buf(),
                            Err(_) => continue,
                        };
                        let path_str = path_buf.to_string_lossy().replace('\\', "/");

                        if !is_searchable_text(&path_buf) {
                            continue;
                        }

                        let reader = BufReader::new(entry_res);
                        for (idx, line_res) in reader.lines().enumerate() {
                            if let Ok(line) = line_res {
                                let matches = if case_sensitive {
                                    line.contains(&q_pattern)
                                } else {
                                    line.to_lowercase().contains(&q_pattern)
                                };

                                if matches {
                                    all_hits.push(SearchHit {
                                        location: archive_name.clone(),
                                        inner_file: path_str.clone(),
                                        line_number: idx + 1,
                                        line_content: line.trim().to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(all_hits)
    }

    /// Search across local Antigravity brain directories
    pub fn search_local(
        query: &str,
        paths: &AntigravityPaths,
        case_sensitive: bool,
    ) -> Result<Vec<SearchHit>> {
        let mut hits = Vec::new();
        let brain_dir = paths.brain_dir();
        if !brain_dir.exists() {
            return Ok(hits);
        }

        let db = Database::new(&paths.summaries_db());
        let all_summaries = db.list_conversations().unwrap_or_default();
        let titles_map: std::collections::HashMap<String, String> = all_summaries
            .into_iter()
            .map(|s| (s.conversation_id, s.title))
            .collect();

        let q_pattern = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for entry_res in walkdir::WalkDir::new(&brain_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry_res.file_type().is_file() {
                let path = entry_res.path();
                if is_searchable_text(path) {
                    if let Ok(rel) = path.strip_prefix(&brain_dir) {
                        let conv_id = rel
                            .components()
                            .next()
                            .map(|c| c.as_os_str().to_string_lossy().to_string())
                            .unwrap_or_default();
                        let title = titles_map
                            .get(&conv_id)
                            .cloned()
                            .unwrap_or_else(|| conv_id.clone());
                        let inner_rel = rel
                            .strip_prefix(&conv_id)
                            .unwrap_or(rel)
                            .to_string_lossy()
                            .replace('\\', "/");

                        if let Ok(file) = fs::File::open(path) {
                            let reader = BufReader::new(file);
                            for (idx, line_res) in reader.lines().enumerate() {
                                if let Ok(line) = line_res {
                                    let matches = if case_sensitive {
                                        line.contains(&q_pattern)
                                    } else {
                                        line.to_lowercase().contains(&q_pattern)
                                    };

                                    if matches {
                                        hits.push(SearchHit {
                                            location: format!(
                                                "{} [{}]",
                                                title,
                                                &conv_id[..conv_id.len().min(8)]
                                            ),
                                            inner_file: inner_rel.clone(),
                                            line_number: idx + 1,
                                            line_content: line.trim().to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(hits)
    }

    /// Renders search hits with highlighted terms
    pub fn print_hits(query: &str, hits: &[SearchHit]) {
        println!();
        println!(
            "{} Found {} matches for '{}'",
            "● Search Results:".bold().cyan(),
            hits.len().to_string().green().bold(),
            query.yellow().bold()
        );
        println!(
            "{}",
            "──────────────────────────────────────────────────────────────────".dimmed()
        );

        if hits.is_empty() {
            println!("{}", "No occurrences found.".dimmed());
            println!();
            return;
        }

        let mut current_loc = String::new();
        for hit in hits {
            if hit.location != current_loc {
                current_loc = hit.location.clone();
                println!(
                    "\n  {} ── {}",
                    current_loc.bold().white(),
                    hit.inner_file.dimmed()
                );
            }

            let highlighted = highlight_substring(&hit.line_content, query);
            println!(
                "    {}: {}",
                hit.line_number.to_string().cyan(),
                highlighted
            );
        }
        println!();
    }
}

fn highlight_substring(line: &str, query: &str) -> String {
    let lower_line = line.to_lowercase();
    let lower_q = query.to_lowercase();

    if let Some(pos) = lower_line.find(&lower_q) {
        let before = &line[..pos];
        let matched = &line[pos..pos + query.len()];
        let after = &line[pos + query.len()..];
        format!("{}{}{}", before, matched.bold().yellow(), after)
    } else {
        line.to_string()
    }
}

fn is_searchable_text(path: &Path) -> bool {
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
