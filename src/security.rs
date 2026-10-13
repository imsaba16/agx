use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

/// Maximum size allowed for a single uncompressed file in an archive (100 MB)
pub const DEFAULT_MAX_ENTRY_SIZE: u64 = 100 * 1024 * 1024;
/// Maximum cumulative uncompressed size allowed across the entire archive (2 GB)
pub const DEFAULT_MAX_TOTAL_SIZE: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum number of files allowed in a single archive (5,000 files)
pub const DEFAULT_MAX_FILE_COUNT: usize = 5_000;

/// Compute the hexadecimal SHA-256 hash of a byte slice
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Protects against Zip-Slip / Path Traversal attacks and symlink escapes
pub struct SafePathValidator;

impl SafePathValidator {
    /// Validates that a relative path extracted from an archive does not escape the base directory
    pub fn validate_and_resolve(base_dir: &Path, rel_path: &Path) -> Result<PathBuf> {
        // 1. Component check: reject any ParentDir (..) or RootDir/Prefix components
        for component in rel_path.components() {
            match component {
                Component::ParentDir => {
                    anyhow::bail!(
                        "Security error (Zip-Slip attempt): Path traversal '..' detected in path: {:?}",
                        rel_path
                    );
                }
                Component::RootDir | Component::Prefix(_) => {
                    anyhow::bail!(
                        "Security error (Zip-Slip attempt): Absolute path detected in path: {:?}",
                        rel_path
                    );
                }
                _ => {}
            }
        }

        let joined = base_dir.join(rel_path);

        // 2. Canonical containment check
        let clean_base = crate::remapper::clean_path(base_dir);
        if joined.exists() {
            let clean_joined = crate::remapper::clean_path(&joined);
            if !clean_joined.starts_with(&clean_base) {
                anyhow::bail!(
                    "Security error: Destination path {:?} escapes extraction base {:?} via symlink",
                    joined,
                    base_dir
                );
            }
        } else if let Some(parent) = joined.parent() {
            if parent.exists() {
                let clean_parent = crate::remapper::clean_path(parent);
                if !clean_parent.starts_with(&clean_base) {
                    anyhow::bail!(
                        "Security error: Destination path {:?} escapes extraction base {:?}",
                        joined,
                        base_dir
                    );
                }
            }
        }

        Ok(joined)
    }
}

/// Guard against decompression bombs and excessive resource consumption
pub struct TarLimitGuard {
    pub max_entry_size: u64,
    pub max_total_size: u64,
    pub max_file_count: usize,
    pub current_total_size: u64,
    pub current_file_count: usize,
}

impl Default for TarLimitGuard {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_ENTRY_SIZE,
            DEFAULT_MAX_TOTAL_SIZE,
            DEFAULT_MAX_FILE_COUNT,
        )
    }
}

impl TarLimitGuard {
    pub fn new(max_entry_size: u64, max_total_size: u64, max_file_count: usize) -> Self {
        Self {
            max_entry_size,
            max_total_size,
            max_file_count,
            current_total_size: 0,
            current_file_count: 0,
        }
    }

    pub fn check_and_add(&mut self, entry_size: u64) -> Result<()> {
        if entry_size > self.max_entry_size {
            anyhow::bail!(
                "Security error: Entry size ({} bytes) exceeds safety limit of {} bytes (decompression bomb protection)",
                entry_size,
                self.max_entry_size
            );
        }

        self.current_file_count += 1;
        if self.current_file_count > self.max_file_count {
            anyhow::bail!(
                "Security error: Archive exceeds maximum allowed file count limit of {} files",
                self.max_file_count
            );
        }

        self.current_total_size += entry_size;
        if self.current_total_size > self.max_total_size {
            anyhow::bail!(
                "Security error: Cumulative archive size ({} bytes) exceeds limit of {} bytes",
                self.current_total_size,
                self.max_total_size
            );
        }

        Ok(())
    }
}

/// Normalizes file permissions, stripping setuid/setgid/sticky bits
pub fn sanitize_mode(mode: u32, is_dir: bool) -> u32 {
    let safe_bits = mode & 0o777; // strip setuid, setgid, sticky
    if is_dir || (safe_bits & 0o111) != 0 {
        0o755 // directories and executable scripts
    } else {
        0o644 // standard non-executable read/write
    }
}

/// Verification report returned by `agx verify`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationReport {
    pub is_valid: bool,
    pub total_files_checked: usize,
    pub matched_checksums: usize,
    pub mismatched_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compute_sha256() {
        let data = b"hello world";
        let hash = compute_sha256(data);
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_zip_slip_rejection() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        // 1. ParentDir traversal attempt
        let evil1 = Path::new("../../etc/passwd");
        assert!(SafePathValidator::validate_and_resolve(base, evil1).is_err());

        let evil2 = Path::new("sub/dir/../../../secret.txt");
        assert!(SafePathValidator::validate_and_resolve(base, evil2).is_err());

        // 2. Absolute path attempt
        let evil3 = Path::new("/etc/hosts");
        assert!(SafePathValidator::validate_and_resolve(base, evil3).is_err());

        // 3. Legitimate path
        let ok_path = Path::new("brain/implementation_plan.md");
        let resolved = SafePathValidator::validate_and_resolve(base, ok_path).unwrap();
        assert!(resolved.starts_with(base));
    }

    #[test]
    fn test_tar_limit_guard() {
        let mut guard = TarLimitGuard::new(1000, 5000, 3);

        // Within limits
        assert!(guard.check_and_add(500).is_ok());
        assert!(guard.check_and_add(400).is_ok());

        // File size exceeds entry limit
        assert!(guard.check_and_add(1500).is_err());

        // File count limit reached (limit 3, added 2 so 1 more is ok, next fails)
        assert!(guard.check_and_add(100).is_ok());
        assert!(guard.check_and_add(50).is_err());
    }

    #[test]
    fn test_sanitize_mode() {
        // Strip setuid (0o4755 -> 0o755)
        assert_eq!(sanitize_mode(0o4755, false), 0o755);
        // Strip setgid on normal file (0o2644 -> 0o644)
        assert_eq!(sanitize_mode(0o2644, false), 0o644);
        // Directories always 0o755
        assert_eq!(sanitize_mode(0o777, true), 0o755);
    }
}
