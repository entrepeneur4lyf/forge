use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::anyhow;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn new(workspace_id: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(
            PathBuf::from(workspace_id)
                .join("state.vscdb")
                .to_string_lossy()
                .to_string(),
        )?;

        Ok(Self { conn })
    }
    pub fn extract_focused_file(&self) -> anyhow::Result<PathBuf> {
        let key = "workbench.explorer.treeViewState";
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM ItemTable WHERE key = ?1")?;
        let value: Option<String> = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .optional()?;

        if let Some(value) = value {
            return Ok(PathBuf::from(focused_file_path(&value)?));
        }

        Err(anyhow!("Focused file not found"))
    }

    pub fn extract_active_files(&self) -> anyhow::Result<HashSet<PathBuf>> {
        let key = "memento/workbench.parts.editor";
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM ItemTable WHERE key = ?1")?;
        let value: Option<String> = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .optional()?;

        if let Some(value) = value {
            return active_files_path(&value);
        }

        Err(anyhow!("Focused file not found"))
    }
}

fn focused_file_path(json_data: &str) -> anyhow::Result<String> {
    // Parse the input JSON into a `serde_json::Value`
    let data: Value = serde_json::from_str(json_data)?;

    // Extract the "focus" array, if it exists
    let focus_array = match data.get("focus") {
        Some(Value::Array(arr)) => arr,
        _ => return Err(anyhow!("Invalid focus json")), // "focus" key not found or not an array
    };

    // Get the first item from "focus", ensuring it's a string
    if let Some(Value::String(item)) = focus_array.first() {
        // The string looks like "file://...::file://..."
        // If you want the second half (the actual file path), split by "::"
        return if let Some(idx) = item.find("::") {
            let second_part = &item[idx + 2..]; // after the "::"
            Ok(second_part
                .strip_prefix("file://")
                .unwrap_or(second_part)
                .to_string())
        } else {
            // If there's no "::", just return the entire string
            Ok(item.strip_prefix("file://").unwrap_or(item).to_string())
        };
    }

    // No first item in the array or it's not a string
    Ok("".to_string())
}

fn active_files_path(json_data: &str) -> anyhow::Result<HashSet<PathBuf>> {
    let parsed: Value = serde_json::from_str(json_data).expect("Invalid JSON");
    let values = jsonpath_lib::Selector::new()
        .str_path("$['editorpart.state'].serializedGrid.root.data[0].data.editors[*].value")?
        .value(&parsed)
        .select()?
        .into_iter()
        .filter_map(|v| v.as_str());
    let mut final_selector = jsonpath_lib::Selector::new();
    let final_selector = final_selector.str_path("$.resourceJSON.fsPath")?;

    let value = values
        .map(serde_json::from_str)
        .filter_map(|v: serde_json::Result<Value>| v.ok())
        .collect::<Vec<_>>();
    let mut ans = HashSet::new();

    for v in value.iter() {
        for v in final_selector.value(v).select()? {
            let val = v.as_str().ok_or(anyhow!("Invalid JSON"))?;
            ans.insert(PathBuf::from(val));
        }
    }
    Ok(ans)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_focused_file_path() {
        let valid_json1 = r#"{
            "focus": ["file:///home/user/project/main.rs::file:///home/user/project/lib.rs"]
        }"#;
        let valid_json2 = r#"{
            "focus": ["file:///home/user/project/main.rs"]
        }"#;
        let invalid_json = r#"{
            "focus": []
        }"#;

        assert_eq!(
            focused_file_path(valid_json1).unwrap(),
            "/home/user/project/lib.rs"
        );
        assert_eq!(
            focused_file_path(valid_json2).unwrap(),
            "/home/user/project/main.rs"
        );
        assert_eq!("", focused_file_path(invalid_json).unwrap());
    }
}
