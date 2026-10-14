use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AntigravityPaths {
    pub base_dir: PathBuf,
}

impl AntigravityPaths {
    pub fn discover(custom_dir: Option<PathBuf>) -> Result<Self> {
        let base_dir = if let Some(dir) = custom_dir {
            dir
        } else if let Ok(env_dir) = std::env::var("ANTIGRAVITY_HOME") {
            PathBuf::from(env_dir)
        } else {
            let home = dirs::home_dir().context("Could not determine user home directory")?;
            let standard_path = home.join(".gemini").join("antigravity");

            // On Windows, check both %USERPROFILE%/.gemini/antigravity and %APPDATA%/Gemini/antigravity
            if cfg!(windows) && !standard_path.exists() {
                if let Some(appdata) = dirs::data_dir() {
                    let alt_path = appdata.join("Gemini").join("antigravity");
                    if alt_path.exists() {
                        alt_path
                    } else {
                        standard_path
                    }
                } else {
                    standard_path
                }
            } else {
                standard_path
            }
        };

        Ok(Self { base_dir })
    }

    pub fn summaries_db(&self) -> PathBuf {
        self.base_dir.join("conversation_summaries.db")
    }

    pub fn conversations_dir(&self) -> PathBuf {
        self.base_dir.join("conversations")
    }

    pub fn brain_dir(&self) -> PathBuf {
        self.base_dir.join("brain")
    }

    #[allow(dead_code)]
    pub fn mcp_dir(&self) -> PathBuf {
        self.base_dir.join("mcp")
    }

    pub fn conversation_db(&self, conversation_id: &str) -> PathBuf {
        self.conversations_dir()
            .join(format!("{}.db", conversation_id))
    }

    pub fn conversation_brain(&self, conversation_id: &str) -> PathBuf {
        self.brain_dir().join(conversation_id)
    }

    #[allow(dead_code)]
    pub fn check_exists(&self) -> bool {
        self.base_dir.exists() && self.summaries_db().exists()
    }

    pub fn ide_app_storage(&self) -> Option<PathBuf> {
        let direct_candidates = vec![
            dirs::data_dir().map(|d| d.join("Antigravity").join("app_storage.json")),
            dirs::config_dir().map(|d| d.join("Antigravity").join("app_storage.json")),
            dirs::home_dir().map(|h| h.join(".config").join("Antigravity").join("app_storage.json")),
            dirs::home_dir().map(|h| {
                h.join("Library")
                    .join("Application Support")
                    .join("Antigravity")
                    .join("app_storage.json")
            }),
        ];

        for candidate in direct_candidates.into_iter().flatten() {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        #[cfg(target_os = "macos")]
        return dirs::home_dir().map(|h| {
            h.join("Library/Application Support/Antigravity/app_storage.json")
        });
        #[cfg(target_os = "windows")]
        return dirs::config_dir().map(|d| d.join("Antigravity").join("app_storage.json"));
        #[cfg(target_os = "linux")]
        return dirs::config_dir().map(|d| d.join("Antigravity").join("app_storage.json"));

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        None
    }
}
