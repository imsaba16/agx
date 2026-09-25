use std::path::{Path, PathBuf};

pub const WORKSPACE_PLACEHOLDER: &str = "{{WORKSPACE_ROOT}}";
pub const WORKSPACE_URI_PLACEHOLDER: &str = "file://{{WORKSPACE_ROOT}}";

pub struct PathRemapper {
    fs_path: String,
    fs_path_alt: String,
    uri_path: String,
}

impl PathRemapper {
    pub fn for_export(workspace_root: &Path) -> Self {
        let cleaned = clean_path(workspace_root);
        let fs_path = cleaned.to_string_lossy().to_string();
        
        // Windows alternate slash variant (e.g. C:/Users/... vs C:\Users\...)
        let fs_path_alt = if cfg!(windows) || fs_path.contains('\\') {
            fs_path.replace('\\', "/")
        } else {
            fs_path.clone()
        };

        let uri_path = to_file_uri(&cleaned);

        Self {
            fs_path,
            fs_path_alt,
            uri_path,
        }
    }

    pub fn remap_export(&self, content: &str) -> String {
        let mut result = content.replace(&self.uri_path, WORKSPACE_URI_PLACEHOLDER);
        
        // Also try matching standard file:/// without extra slashes if different
        let alt_uri = format!("file://{}", self.fs_path_alt);
        if alt_uri != self.uri_path {
            result = result.replace(&alt_uri, WORKSPACE_URI_PLACEHOLDER);
        }

        // Replace both backslash and forward slash variants of the local path
        result = result.replace(&self.fs_path, WORKSPACE_PLACEHOLDER);
        result = result.replace(&self.fs_path_alt, WORKSPACE_PLACEHOLDER);
        result
    }

    pub fn remap_import(content: &str, target_workspace: &Path) -> String {
        let cleaned = clean_path(target_workspace);
        let target_uri = to_file_uri(&cleaned);
        let target_fs = cleaned.to_string_lossy().to_string();

        // 1. First replace file://{{WORKSPACE_ROOT}} with standard URI
        let mut result = content.replace(WORKSPACE_URI_PLACEHOLDER, &target_uri);

        // 2. Then replace {{WORKSPACE_ROOT}}
        // On Windows, if following path is /something, convert slashes appropriately
        if cfg!(windows) {
            // Replace {{WORKSPACE_ROOT}}/foo with C:\path\foo
            let re_slash = format!("{}/", WORKSPACE_PLACEHOLDER);
            let target_with_slash = format!("{}\\", target_fs);
            result = result.replace(&re_slash, &target_with_slash);
        }

        result = result.replace(WORKSPACE_PLACEHOLDER, &target_fs);
        result
    }

    pub fn remap_workspace_uris_export(&self, uris_json: &str) -> String {
        self.remap_export(uris_json)
    }

    pub fn remap_workspace_uris_import(uris_json: &str, target_workspace: &Path) -> String {
        Self::remap_import(uris_json, target_workspace)
    }
}

/// Strip Windows verbatim prefix (\\?\) if present
pub fn clean_path(path: &Path) -> PathBuf {
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p
    }
}

/// Create a valid RFC file URI across Unix and Windows
pub fn to_file_uri(path: &Path) -> String {
    let s = clean_path(path).to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{}", s)
    } else {
        // Windows drive letter like C:/Users/... -> file:///C:/Users/...
        format!("file:///{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_export_to_windows_import() {
        let remapper = PathRemapper {
            fs_path: "/Users/sanjays/project".to_string(),
            fs_path_alt: "/Users/sanjays/project".to_string(),
            uri_path: "file:///Users/sanjays/project".to_string(),
        };

        let mac_text = "See file:///Users/sanjays/project/src/main.rs and /Users/sanjays/project/Cargo.toml";
        let exported = remapper.remap_export(mac_text);

        assert_eq!(
            exported,
            "See file://{{WORKSPACE_ROOT}}/src/main.rs and {{WORKSPACE_ROOT}}/Cargo.toml"
        );

        // Simulate importing on Windows
        let windows_target = PathBuf::from("C:\\Users\\Alice\\project");
        let cleaned = clean_path(&windows_target);
        let target_uri = to_file_uri(&cleaned);
        assert_eq!(target_uri, "file:///C:/Users/Alice/project");

        let imported = exported.replace(WORKSPACE_URI_PLACEHOLDER, &target_uri);
        assert!(imported.contains("file:///C:/Users/Alice/project/src/main.rs"));
    }

    #[test]
    fn test_windows_export_to_unix_import() {
        let remapper = PathRemapper {
            fs_path: "C:\\Users\\Alice\\project".to_string(),
            fs_path_alt: "C:/Users/Alice/project".to_string(),
            uri_path: "file:///C:/Users/Alice/project".to_string(),
        };

        let win_text = "See file:///C:/Users/Alice/project/src/main.rs and C:\\Users\\Alice\\project\\Cargo.toml";
        let exported = remapper.remap_export(win_text);

        assert!(exported.contains("file://{{WORKSPACE_ROOT}}/src/main.rs"));
        assert!(exported.contains("{{WORKSPACE_ROOT}}\\Cargo.toml") || exported.contains("{{WORKSPACE_ROOT}}/Cargo.toml"));

        // Simulate importing on Linux
        let linux_target = PathBuf::from("/home/bob/project");
        let imported = PathRemapper::remap_import(&exported, &linux_target);
        assert!(imported.contains("file:///home/bob/project/src/main.rs"));
    }
}
