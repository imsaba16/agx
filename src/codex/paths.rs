use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CodexPaths {
    pub base_dir: PathBuf,
}

impl CodexPaths {
    pub fn discover() -> Result<Self> {
        let base_dir = if let Ok(env_dir) = std::env::var("CODEX_HOME") {
            PathBuf::from(env_dir)
        } else {
            let home = dirs::home_dir().context("Could not determine user home directory")?;
            home.join(".codex")
        };

        Ok(Self { base_dir })
    }

    pub fn is_installed(&self) -> bool {
        self.base_dir.exists() || self.find_binary().is_some()
    }

    pub fn find_binary(&self) -> Option<PathBuf> {
        // 1. Check PATH environment variable
        if let Ok(path_var) = std::env::var("PATH") {
            for part in std::env::split_paths(&path_var) {
                let candidate = part.join(if cfg!(windows) { "codex.exe" } else { "codex" });
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }

        // 2. Common fallback: ~/.local/bin/codex
        if let Some(home) = dirs::home_dir() {
            let local_bin = home.join(".local").join("bin").join("codex");
            if local_bin.exists() {
                return Some(local_bin);
            }
        }

        None
    }

    /// Finds the newest state SQLite database (e.g., state_5.sqlite, state_6.sqlite)
    pub fn state_db(&self) -> Option<PathBuf> {
        if !self.base_dir.exists() {
            return None;
        }

        // Check state_5.sqlite first (standard current version)
        let standard_state_5 = self.base_dir.join("state_5.sqlite");
        if standard_state_5.exists() {
            return Some(standard_state_5);
        }

        // Otherwise scan for highest state_*.sqlite
        let mut best: Option<(u32, PathBuf)> = None;
        if let Ok(entries) = std::fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("state_") && name.ends_with(".sqlite") {
                    if let Some(num_str) = name.strip_prefix("state_").and_then(|s| s.strip_suffix(".sqlite")) {
                        if let Ok(ver) = num_str.parse::<u32>() {
                            if best.as_ref().map(|(b, _)| ver > *b).unwrap_or(true) {
                                best = Some((ver, entry.path()));
                            }
                        }
                    }
                }
            }
        }

        best.map(|(_, path)| path)
    }

    pub fn config_file(&self) -> PathBuf {
        self.base_dir.join("config.toml")
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.base_dir.join("skills")
    }

    pub fn system_skills_dir(&self) -> PathBuf {
        self.skills_dir().join(".system")
    }

    pub fn memories_db(&self) -> PathBuf {
        self.base_dir.join("memories_1.sqlite")
    }
}
