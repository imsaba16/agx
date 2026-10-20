use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct McpServerDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AntigravityMcpConfigFile {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: BTreeMap<String, AntigravityMcpServerDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AntigravityMcpServerDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(rename = "serverUrl", skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(rename = "authProviderType", skip_serializing_if = "Option::is_none")]
    pub auth_provider_type: Option<String>,
}

pub struct McpBridge;

impl McpBridge {
    /// Reads MCP servers from Antigravity configuration file (e.g. ~/.gemini/config/mcp_config.json)
    pub fn read_antigravity_mcp(path: &Path) -> Result<Vec<McpServerDef>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Antigravity MCP config at {:?}", path))?;
        let config: AntigravityMcpConfigFile = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse Antigravity MCP JSON from {:?}", path))?;

        let mut servers = Vec::new();
        for (name, s) in config.mcp_servers {
            servers.push(McpServerDef {
                name,
                command: s.command,
                args: s.args,
                env: s.env,
                url: s.server_url,
            });
        }

        Ok(servers)
    }

    /// Reads MCP servers from Codex configuration file (~/.codex/config.toml)
    pub fn read_codex_mcp(path: &Path) -> Result<Vec<McpServerDef>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read Codex config at {:?}", path))?;
        let val: toml::Value = toml::from_str(&content)
            .with_context(|| format!("Failed to parse Codex TOML from {:?}", path))?;

        let mut servers = Vec::new();
        if let Some(mcp_table) = val.get("mcp_servers").and_then(|v| v.as_table()) {
            for (name, server_val) in mcp_table {
                let command = server_val.get("command").and_then(|c| c.as_str()).map(String::from);
                let args = server_val
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| item.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut env = BTreeMap::new();
                if let Some(env_table) = server_val.get("env").and_then(|e| e.as_table()) {
                    for (k, v) in env_table {
                        if let Some(v_str) = v.as_str() {
                            env.insert(k.clone(), v_str.to_string());
                        }
                    }
                }
                let url = server_val.get("url").and_then(|u| u.as_str()).map(String::from);

                servers.push(McpServerDef {
                    name: name.clone(),
                    command,
                    args,
                    env,
                    url,
                });
            }
        }

        Ok(servers)
    }

    /// Syncs servers from Antigravity to Codex config.toml without deleting existing non-MCP settings
    pub fn sync_antigravity_to_codex(antigravity_json: &Path, codex_toml: &Path) -> Result<usize> {
        let servers = Self::read_antigravity_mcp(antigravity_json)?;
        if servers.is_empty() {
            return Ok(0);
        }

        let mut toml_val: toml::Value = if codex_toml.exists() {
            let content = std::fs::read_to_string(codex_toml)?;
            toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()))
        } else {
            toml::Value::Table(toml::map::Map::new())
        };

        let root_table = toml_val
            .as_table_mut()
            .context("Invalid root TOML format in Codex config")?;

        let mcp_servers_entry = root_table
            .entry("mcp_servers")
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

        let mcp_table = mcp_servers_entry
            .as_table_mut()
            .context("mcp_servers is not a table in Codex config")?;

        let mut synced_count = 0;
        for s in servers {
            let mut server_table = toml::map::Map::new();
            if let Some(cmd) = s.command {
                server_table.insert("command".to_string(), toml::Value::String(cmd));
            }
            if !s.args.is_empty() {
                let args_vals = s.args.into_iter().map(toml::Value::String).collect();
                server_table.insert("args".to_string(), toml::Value::Array(args_vals));
            }
            if !s.env.is_empty() {
                let mut env_map = toml::map::Map::new();
                for (k, v) in s.env {
                    env_map.insert(k, toml::Value::String(v));
                }
                server_table.insert("env".to_string(), toml::Value::Table(env_map));
            }
            if let Some(url) = s.url {
                server_table.insert("url".to_string(), toml::Value::String(url));
            }

            mcp_table.insert(s.name, toml::Value::Table(server_table));
            synced_count += 1;
        }

        let updated_toml = toml::to_string_pretty(&toml_val)?;
        if let Some(parent) = codex_toml.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(codex_toml, updated_toml)?;

        Ok(synced_count)
    }

    /// Syncs servers from Codex config.toml to Antigravity mcp_config.json
    pub fn sync_codex_to_antigravity(codex_toml: &Path, antigravity_json: &Path) -> Result<usize> {
        let servers = Self::read_codex_mcp(codex_toml)?;
        if servers.is_empty() {
            return Ok(0);
        }

        let mut config: AntigravityMcpConfigFile = if antigravity_json.exists() {
            let content = std::fs::read_to_string(antigravity_json)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AntigravityMcpConfigFile::default()
        };

        let mut synced_count = 0;
        for s in servers {
            config.mcp_servers.insert(
                s.name,
                AntigravityMcpServerDef {
                    command: s.command,
                    args: s.args,
                    env: s.env,
                    server_url: s.url,
                    auth_provider_type: None,
                },
            );
            synced_count += 1;
        }

        let updated_json = serde_json::to_string_pretty(&config)?;
        if let Some(parent) = antigravity_json.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(antigravity_json, updated_json)?;

        Ok(synced_count)
    }

    /// Automatically locates the global Antigravity MCP config path
    pub fn find_antigravity_mcp_config() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let candidates = vec![
            home.join(".gemini").join("config").join("mcp_config.json"),
            home.join(".gemini").join("antigravity").join("mcp_config.json"),
        ];

        for c in candidates {
            if c.exists() {
                return Some(c);
            }
        }

        // Default path if creating new
        Some(home.join(".gemini").join("config").join("mcp_config.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mcp_sync_bidirectional() {
        let dir = tempdir().unwrap();
        let ag_json = dir.path().join("mcp_config.json");
        let cdx_toml = dir.path().join("config.toml");

        // 1. Create sample Antigravity MCP config
        let sample_ag = r#"{
            "mcpServers": {
                "test-server": {
                    "command": "npx",
                    "args": ["-y", "some-tool"],
                    "serverUrl": "https://example.com/mcp"
                }
            }
        }"#;
        std::fs::write(&ag_json, sample_ag).unwrap();

        // 2. Sync to Codex
        let synced = McpBridge::sync_antigravity_to_codex(&ag_json, &cdx_toml).unwrap();
        assert_eq!(synced, 1);

        // 3. Read from Codex
        let cdx_servers = McpBridge::read_codex_mcp(&cdx_toml).unwrap();
        assert_eq!(cdx_servers.len(), 1);
        assert_eq!(cdx_servers[0].name, "test-server");
        assert_eq!(cdx_servers[0].command.as_deref(), Some("npx"));
        assert_eq!(cdx_servers[0].url.as_deref(), Some("https://example.com/mcp"));

        // 4. Modify Codex config and sync back
        let new_ag_json = dir.path().join("mcp_config_new.json");
        let synced_back = McpBridge::sync_codex_to_antigravity(&cdx_toml, &new_ag_json).unwrap();
        assert_eq!(synced_back, 1);

        let ag_servers = McpBridge::read_antigravity_mcp(&new_ag_json).unwrap();
        assert_eq!(ag_servers.len(), 1);
        assert_eq!(ag_servers[0].name, "test-server");
    }
}
