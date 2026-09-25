use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs;
use std::path::Path;

pub struct IgnoreFilter {
    glob_set: GlobSet,
    compact: bool,
}

impl IgnoreFilter {
    pub fn new(
        compact: bool,
        custom_ignore_file: Option<&Path>,
        workspace_root: Option<&Path>,
    ) -> Self {
        let mut builder = GlobSetBuilder::new();

        // 1. Default system ignores
        let default_patterns = [
            "**/.DS_Store",
            "**/Thumbs.db",
            ".tempmediaStorage/**",
            "**/.tempmediaStorage/**",
        ];

        for pattern in default_patterns {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
            }
        }

        // 2. Compact mode patterns
        if compact {
            let compact_patterns = [
                ".system_generated/steps/**",
                "**/.system_generated/steps/**",
                ".system_generated/messages/**",
                "**/.system_generated/messages/**",
                ".system_generated/tasks/**",
                "**/.system_generated/tasks/**",
                "**/.system_generated/logs/transcript_full.jsonl",
                ".user_uploaded/**",
                "**/.user_uploaded/**",
                "scratch/**",
                "**/scratch/**",
            ];

            for pattern in compact_patterns {
                if let Ok(glob) = Glob::new(pattern) {
                    builder.add(glob);
                }
            }
        }

        // 3. Custom ignore file or workspace .agignore
        let ignore_path = custom_ignore_file
            .map(|p| p.to_path_buf())
            .or_else(|| {
                workspace_root.map(|ws| ws.join(".agignore"))
            })
            .or_else(|| {
                dirs::home_dir().map(|h| h.join(".agignore"))
            });

        if let Some(ref path) = ignore_path {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() || trimmed.starts_with('#') {
                            continue;
                        }

                        // Support directory trailing slash (e.g. "scratch/" -> "scratch/**")
                        let pattern = if trimmed.ends_with('/') {
                            format!("{}**", trimmed)
                        } else {
                            trimmed.to_string()
                        };

                        // Add both root and recursive variant if not rooted
                        if !pattern.starts_with('/') && !pattern.starts_with("**/") {
                            if let Ok(glob) = Glob::new(&format!("**/{}", pattern)) {
                                builder.add(glob);
                            }
                        }

                        if let Ok(glob) = Glob::new(&pattern) {
                            builder.add(glob);
                        }
                    }
                }
            }
        }

        let glob_set = builder.build().unwrap_or_else(|_| GlobSet::empty());

        Self { glob_set, compact }
    }

    pub fn is_ignored(&self, relative_path: &Path) -> bool {
        let path_str = relative_path.to_string_lossy().replace('\\', "/");
        self.glob_set.is_match(&path_str)
    }

    pub fn is_compact(&self) -> bool {
        self.compact
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_compact_filtering() {
        let filter = IgnoreFilter::new(true, None, None);

        // Core files should NOT be ignored
        assert!(!filter.is_ignored(&PathBuf::from("implementation_plan.md")));
        assert!(!filter.is_ignored(&PathBuf::from("walkthrough.md")));
        assert!(!filter.is_ignored(&PathBuf::from(".system_generated/logs/transcript.jsonl")));

        // Ephemeral steps and heavy scratch should be ignored
        assert!(filter.is_ignored(&PathBuf::from(".system_generated/steps/001.json")));
        assert!(filter.is_ignored(&PathBuf::from(".system_generated/messages/msg1.json")));
        assert!(filter.is_ignored(&PathBuf::from(".system_generated/tasks/task-62.log")));
        assert!(filter.is_ignored(&PathBuf::from(".system_generated/logs/transcript_full.jsonl")));
        assert!(filter.is_ignored(&PathBuf::from(".user_uploaded/screenshot.png")));
        assert!(filter.is_ignored(&PathBuf::from("scratch/test_script.py")));
    }

    #[test]
    fn test_custom_agignore_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let agignore_path = temp_dir.path().join(".agignore");
        fs::write(
            &agignore_path,
            "# Ignore video screen recordings\n*.mp4\nscratch/tmp_*\n.tempmediaStorage/\n",
        )
        .unwrap();

        let filter = IgnoreFilter::new(false, Some(&agignore_path), None);

        assert!(filter.is_ignored(&PathBuf::from(".user_uploaded/recording.mp4")));
        assert!(filter.is_ignored(&PathBuf::from("scratch/tmp_debug.py")));
        assert!(!filter.is_ignored(&PathBuf::from("scratch/keep_me.py")));
        assert!(!filter.is_ignored(&PathBuf::from("implementation_plan.md")));
    }
}
