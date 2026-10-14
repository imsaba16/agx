use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct IdeStorage;

impl IdeStorage {
    /// Registers a project ID in Antigravity's IDE app_storage.json so that
    /// the IDE project manager surfaces the project and groups all its conversations.
    pub fn register_project(app_storage_path: &Path, project_id: &str) -> Result<()> {
        if project_id.trim().is_empty() {
            return Ok(());
        }

        if !app_storage_path.exists() {
            // If parent directory exists, we can create a minimal app_storage.json
            if let Some(parent) = app_storage_path.parent() {
                if !parent.exists() {
                    return Ok(());
                }
            }
            let initial = serde_json::json!({
                "projectsOrder": serde_json::to_string(&vec![project_id])?,
                "lastCreatedProjectId": project_id
            });
            fs::write(app_storage_path, serde_json::to_string_pretty(&initial)?)?;
            return Ok(());
        }

        let content = fs::read_to_string(app_storage_path)?;
        let mut storage: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let mut changed = false;

        // 1. Update projectsOrder (stringified JSON array of UUIDs)
        if let Some(order_val) = storage.get("projectsOrder") {
            let mut list: Vec<String> = if let Some(order_str) = order_val.as_str() {
                serde_json::from_str(order_str).unwrap_or_default()
            } else if let Some(order_arr) = order_val.as_array() {
                order_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            };

            if !list.contains(&project_id.to_string()) {
                list.push(project_id.to_string());
                if order_val.is_string() {
                    storage["projectsOrder"] = serde_json::Value::String(serde_json::to_string(&list)?);
                } else {
                    storage["projectsOrder"] = serde_json::json!(list);
                }
                changed = true;
            }
        } else {
            let list = vec![project_id.to_string()];
            storage["projectsOrder"] = serde_json::Value::String(serde_json::to_string(&list)?);
            changed = true;
        }

        // 2. Set lastCreatedProjectId if changed
        if storage.get("lastCreatedProjectId").and_then(|v| v.as_str()) != Some(project_id) {
            storage["lastCreatedProjectId"] = serde_json::json!(project_id);
            changed = true;
        }

        if changed {
            let updated = serde_json::to_string_pretty(&storage)?;
            fs::write(app_storage_path, updated)?;
        }

        Ok(())
    }

    /// Read the list of project IDs from app_storage.json
    pub fn get_projects_order(app_storage_path: &Path) -> Result<Vec<String>> {
        if !app_storage_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(app_storage_path)?;
        let storage: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(order_val) = storage.get("projectsOrder") {
            let list: Vec<String> = if let Some(order_str) = order_val.as_str() {
                serde_json::from_str(order_str).unwrap_or_default()
            } else if let Some(order_arr) = order_val.as_array() {
                order_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            };
            Ok(list)
        } else {
            Ok(Vec::new())
        }
    }
}
