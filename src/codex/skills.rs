use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub is_symlink: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsSyncReport {
    pub synced: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

pub struct SkillsBridge;

impl SkillsBridge {
    /// Extracts frontmatter metadata (name, description) from SKILL.md
    pub fn parse_skill_md(skill_md_path: &Path) -> (String, String) {
        let default_name = skill_md_path
            .parent()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if let Ok(content) = std::fs::read_to_string(skill_md_path) {
            let mut in_frontmatter = false;
            let mut name = None;
            let mut desc = None;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed == "---" {
                    if !in_frontmatter {
                        in_frontmatter = true;
                        continue;
                    } else {
                        break;
                    }
                }

                if in_frontmatter {
                    if let Some(rest) = trimmed.strip_prefix("name:") {
                        name = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
                    } else if let Some(rest) = trimmed.strip_prefix("description:") {
                        desc = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
                    }
                }
            }

            let final_name = name.unwrap_or(default_name);
            let final_desc = desc.unwrap_or_default();
            (final_name, final_desc)
        } else {
            (default_name, String::new())
        }
    }

    /// Discovers all valid skills in a folder (directories containing SKILL.md)
    pub fn list_skills(base_dir: &Path, include_system: bool) -> Result<Vec<SkillInfo>> {
        if !base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();
        for entry in std::fs::read_dir(base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !include_system && path.file_name().map(|n| n == ".system").unwrap_or(false) {
                continue;
            }

            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    let (name, desc) = Self::parse_skill_md(&skill_md);
                    let is_symlink = path.is_symlink();
                    skills.push(SkillInfo {
                        name,
                        description: desc,
                        path,
                        is_symlink,
                    });
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Synchronizes skills from source directory to target directory
    pub fn sync_skills(
        source_dir: &Path,
        target_dir: &Path,
        use_symlinks: bool,
        overwrite: bool,
    ) -> Result<SkillsSyncReport> {
        let mut report = SkillsSyncReport::default();
        if !source_dir.exists() {
            return Ok(report);
        }

        std::fs::create_dir_all(target_dir)?;
        let source_skills = Self::list_skills(source_dir, false)?;

        for skill in source_skills {
            let dest_path = target_dir.join(&skill.name);

            if dest_path.exists() {
                if !overwrite {
                    report.skipped.push(skill.name.clone());
                    continue;
                } else {
                    // Remove existing destination
                    if dest_path.is_symlink() || dest_path.is_file() {
                        let _ = std::fs::remove_file(&dest_path);
                    } else if dest_path.is_dir() {
                        let _ = std::fs::remove_dir_all(&dest_path);
                    }
                }
            }

            let sync_res = if use_symlinks {
                Self::create_symlink(&skill.path, &dest_path)
            } else {
                Self::copy_dir_recursive(&skill.path, &dest_path)
            };

            match sync_res {
                Ok(_) => report.synced.push(skill.name),
                Err(e) => report.failed.push((skill.name, e.to_string())),
            }
        }

        Ok(report)
    }

    fn create_symlink(source: &Path, target: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, target)
                .with_context(|| format!("Failed to create symlink from {:?} to {:?}", source, target))?;
            Ok(())
        }

        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_dir(source, target).is_err() {
                // Fallback to recursive copy on Windows if developer mode symlink is not enabled
                Self::copy_dir_recursive(source, target)?;
            }
            Ok(())
        }
    }

    fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
        std::fs::create_dir_all(target)?;
        for entry in WalkDir::new(source).min_depth(1) {
            let entry = entry?;
            let rel = entry.path().strip_prefix(source)?;
            let dest = target.join(rel);

            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dest)?;
            } else {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(entry.path(), &dest)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_skills_listing_and_copy_sync() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("source_skills");
        let dst_dir = dir.path().join("dest_skills");

        let my_skill = src_dir.join("test-skill");
        std::fs::create_dir_all(&my_skill).unwrap();
        let skill_md_content = "---\nname: test-skill\ndescription: A unit test skill\n---\n# Test";
        std::fs::write(my_skill.join("SKILL.md"), skill_md_content).unwrap();

        let skills = SkillsBridge::list_skills(&src_dir, false).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test-skill");
        assert_eq!(skills[0].description, "A unit test skill");

        let report = SkillsBridge::sync_skills(&src_dir, &dst_dir, false, false).unwrap();
        assert_eq!(report.synced.len(), 1);
        assert_eq!(report.synced[0], "test-skill");
        assert!(dst_dir.join("test-skill").join("SKILL.md").exists());
    }
}
