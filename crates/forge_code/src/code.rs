use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail};
use async_trait::async_trait;
use forge_domain::{Ide, IdeRepository, Workspace, WorkspaceId};
use forge_walker::Walker;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sysinfo::System;

/// Represents Visual Studio Code IDE interaction
pub struct Code {
    root_dir: String,
}

impl Code {
    /// Create a new Code instance with a custom working directory
    pub fn new<T: ToString>(cwd: T) -> Self {
        let cwd = cwd.to_string();

        // VS code stores the path without any trailing slashes.
        // We need to canonicalize the path to remove any trailing slashes.
        let cwd = PathBuf::from(&cwd)
            .canonicalize()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or(cwd);

        Self { root_dir: cwd }
    }
}

#[async_trait]
impl IdeRepository for Code {
    async fn get_active_ides(&self) -> anyhow::Result<Vec<Ide>> {
        get_all_vscode_instances(&self.root_dir).await
    }

    async fn get_workspace(&self, ide: &WorkspaceId) -> anyhow::Result<Workspace> {
        get_workspace_inner(ide.clone()).await
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Storage {
    windows_state: WindowState,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WindowState {
    last_active_window: LastActiveWindow,
    opened_windows: Vec<LastActiveWindow>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LastActiveWindow {
    folder: String,
}

async fn extract_workspace_id(args: &[String], cwd: &str, index: usize) -> anyhow::Result<String> {
    let code_dir =
        extract_storage_dir(args).ok_or(anyhow!("Failed to extract storage directory"))?;
    let storage_file = PathBuf::from(format!("{}/User/globalStorage/storage.json", code_dir));
    let storage_json = tokio::fs::read_to_string(storage_file).await?;
    let json: Storage = serde_json::from_str(&storage_json)?;
    let path_buf = PathBuf::from(code_dir.clone())
        .join("User")
        .join("workspaceStorage");
    let cwd = convert_path(cwd);

    // Not sure if matching index is good idea.
    let search_dir = if check_search_dir_condition(json, &cwd, index) {
        cwd
    } else {
        return Err(anyhow!("Project not active in VS code"));
    };

    let hash_file = get_hash(Walker::new(path_buf.clone()), &search_dir, path_buf).await?;

    Ok(hash_file.path)
}

fn check_search_dir_condition(json: Storage, cwd: &str, index: usize) -> bool {
    let a = json
        .windows_state
        .opened_windows
        .iter()
        .enumerate()
        .any(|(i, folder)| {
            i == index
                && folder
                    .folder
                    .strip_prefix("file://")
                    .map(convert_path)
                    .unwrap_or_default()
                    .eq(&cwd)
        });

    let b = json
        .windows_state
        .last_active_window
        .folder
        .strip_prefix("file://")
        .map(convert_path)
        .unwrap_or_default()
        .eq(&cwd);
    a || b
}

fn convert_path(v: &str) -> String {
    convert_path_inner(v, std::env::consts::OS)
}

fn convert_path_inner(v: &str, os: &str) -> String {
    if os == "windows" {
        let v = urlencoding::decode(v)
            .map(|v| v.to_string())
            .unwrap_or(v.to_string());
        let v = v.split(":").last().unwrap_or(&v);
        typed_path::WindowsPath::new(v)
            .with_unix_encoding()
            .to_string()
    } else {
        v.to_string()
    }
}

fn extract_storage_dir(args: &[String]) -> Option<String> {
    args.iter()
        .find_map(|v| find_arg_value(&[v.clone()], "--user-data-dir="))
}

async fn get_all_vscode_instances(cwd: &str) -> anyhow::Result<Vec<Ide>> {
    let mut ans = vec![];
    let mut system = System::new_all();
    system.refresh_all();

    for (i, v) in system
        .processes()
        .values()
        .filter(|process| {
            process.name().eq_ignore_ascii_case("electron") // for linux
                || process.name().eq_ignore_ascii_case("code helper (renderer)")  // for macos
                || process.name().eq_ignore_ascii_case("code.exe") // for windows
        })
        .filter(|process| {
            process
                .cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().contains("vscode-window-config"))
        })
        .enumerate()
    {
        let cmd = v
            .cmd()
            .iter()
            .map(|v| v.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        let working_directory = v
            .cwd()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();

        if let Some(ide) = get_vscode_instance(cmd, working_directory, cwd, i).await {
            ans.push(ide);
        }
    }

    Ok(ans)
}

async fn get_vscode_instance(
    cmd: Vec<String>,
    working_directory: String,
    cwd: &str,
    index: usize,
) -> Option<Ide> {
    if let Ok(workspace_id) = extract_workspace_id(&cmd, cwd, index).await {
        return Some(Ide {
            name: "VS Code".to_string(),
            version: None,
            working_directory: working_directory.into(),
            workspace_id: workspace_id.into(),
        });
    }

    None
}

async fn get_hash(
    walker: Walker,
    cwd: &str,
    workspace_storage_path: PathBuf,
) -> anyhow::Result<forge_walker::File> {
    let dirs = walker
        .with_max_depth(1)
        .get()
        .await?
        .into_iter()
        .map(|mut v| {
            v.path = workspace_storage_path
                .join(v.path)
                .to_string_lossy()
                .to_string();
            v
        })
        .filter(|v| v.is_dir)
        .collect::<HashSet<_>>();

    for dir in dirs {
        if process_workflow_file(Path::new(&dir.path), cwd).await {
            return Ok(dir);
        }
    }

    bail!("Project not found")
}

async fn get_workspace_inner(workspace_id: WorkspaceId) -> anyhow::Result<Workspace> {
    let mut ans = Workspace::default().workspace_id(workspace_id.clone());
    let conn = Connection::open(
        PathBuf::from(workspace_id.as_str())
            .join("state.vscdb")
            .to_string_lossy()
            .to_string(),
    )?;

    ans = ans.focused_file(extract_focused_file(&conn)?);
    ans = ans.opened_files(extract_active_files(&conn)?);
    Ok(ans)
}

fn extract_active_files(conn: &Connection) -> anyhow::Result<HashSet<PathBuf>> {
    let key = "memento/workbench.parts.editor";
    let mut stmt = conn.prepare("SELECT value FROM ItemTable WHERE key = ?1")?;
    let value: Option<String> = stmt
        .query_row(rusqlite::params![key], |row| row.get(0))
        .optional()?;

    if let Some(value) = value {
        return active_files_path(&value);
    }

    Err(anyhow!("Focused file not found"))
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

fn extract_focused_file(connection: &Connection) -> anyhow::Result<PathBuf> {
    let key = "workbench.explorer.treeViewState";
    let mut stmt = connection.prepare("SELECT value FROM ItemTable WHERE key = ?1")?;
    let value: Option<String> = stmt
        .query_row(rusqlite::params![key], |row| row.get(0))
        .optional()?;

    if let Some(value) = value {
        return Ok(PathBuf::from(focused_file_path(&value)?));
    }

    Err(anyhow!("Focused file not found"))
}

async fn process_workflow_file(path: &Path, cwd: &str) -> bool {
    let path = path.join("workspace.json");

    if let Ok(content) = tokio::fs::read_to_string(&path).await {
        let workflow_json: Value = serde_json::from_str(&content).unwrap_or_default();

        if let Some(folder) = workflow_json.get("folder").and_then(|v| v.as_str()) {
            // Remove "file://" prefix
            let project_path = convert_path(folder.strip_prefix("file://").unwrap_or(folder));

            // Check if the project path matches or is a parent of the current working
            // directory
            if cwd.eq(&project_path) {
                return true;
            }
        }
    }

    false
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

fn find_arg_value(cmd: &[String], key: &str) -> Option<String> {
    for arg in cmd {
        if let Some(pos) = arg.find(key) {
            // Extract the substring starting after the key
            let value_with_rest = &arg[pos + key.len()..];

            // Check if the extracted value starts and ends cleanly (handle multi-word
            // paths)
            if value_with_rest.contains(" --") {
                // Extract up to the first occurrence of " --"
                if let Some(end_pos) = value_with_rest.find(" --") {
                    return Some(value_with_rest[..end_pos].to_string());
                }
            } else {
                // If no " --" exists, return the whole value
                return Some(value_with_rest.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_arg_value() {
        let cmd1 = vec![
            "--user-data-dir=/path/to/vscode".to_string(),
            "--another-arg".to_string(),
        ];
        assert_eq!(
            find_arg_value(&cmd1, "--user-data-dir="),
            Some("/path/to/vscode".to_string())
        );

        let cmd2 = vec![
            "some-other-arg".to_string(),
            "--user-data-dir=/another/path --some-other-flag".to_string(),
        ];
        assert_eq!(
            find_arg_value(&cmd2, "--user-data-dir="),
            Some("/another/path".to_string())
        );

        let cmd3 = vec!["--no-matching-arg".to_string()];
        assert_eq!(find_arg_value(&cmd3, "--user-data-dir="), None);
    }

    #[test]
    fn test_convert_path_windows() {
        let test_paths = vec![
            ("file://C%3A/Users/test", "/Users/test"),
            ("/path/to/file", "/path/to/file"),
        ];
        for (input, expected) in test_paths {
            assert_eq!(convert_path_inner(input, "windows"), expected);
        }
    }

    #[test]
    fn test_convert_path_unix() {
        assert_eq!(convert_path("/path/to/file"), "/path/to/file");
    }

    #[tokio::test]
    async fn test_process_workflow_file() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let workspace_path = temp_dir.path().join("workspace.json");

        // Valid workspace JSON
        {
            std::fs::write(
                &workspace_path,
                r#"{"folder": "file:///home/user/project"}"#,
            )
            .expect("Failed to write workspace JSON");

            assert!(process_workflow_file(temp_dir.path(), "/home/user/project").await);
        }

        // Invalid workspace JSON
        {
            std::fs::write(
                &workspace_path,
                r#"{"folder": "file:///unrelated/project"}"#,
            )
            .expect("Failed to write invalid workspace JSON");

            assert!(!process_workflow_file(temp_dir.path(), "/home/user/project").await);
        }
    }

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

#[cfg(test)]
mod partial_integration_tests {
    use tempfile::TempDir;

    use crate::code::get_vscode_instance;

    #[tokio::test]
    async fn test_get_vscode_instance() {
        let dir = TempDir::new().unwrap();

        // Create User directory structure
        std::fs::create_dir_all(dir.path().join("User/globalStorage")).unwrap();
        std::fs::create_dir_all(dir.path().join("User/workspaceStorage/some_hash")).unwrap();

        // Create storage.json
        let storage_json = serde_json::json!({
            "windowsState": {
                "lastActiveWindow": {
                    "folder": "file:///home/foo/RustroverProjects/code-forge"
                },
                "openedWindows": [
                    {
                        "folder": "file:///home/foo/RustroverProjects/code-forge"
                    }
                ]
            }
        });
        std::fs::write(
            dir.path().join("User/globalStorage/storage.json"),
            serde_json::to_string_pretty(&storage_json).unwrap(),
        )
        .unwrap();

        // Create workspace.json
        let workspace_json = serde_json::json!({
            "folder": "file:///home/foo/RustroverProjects/code-forge"
        });
        std::fs::write(
            dir.path()
                .join("User/workspaceStorage/some_hash/workspace.json"),
            serde_json::to_string_pretty(&workspace_json).unwrap(),
        )
        .unwrap();

        let cmd = vec![
            format!(
                "/usr/lib/electron32/electron --user-data-dir={}",
                dir.path().display()
            ),
            "--vscode-window-config".to_string(), // Simulate VSCode window config
        ];

        let working_directory = "/home/foo/RustroverProjects/code-forge".to_string();
        let cwd = "/home/foo/RustroverProjects/code-forge";
        let ans = get_vscode_instance(cmd, working_directory, cwd, 0).await;

        // Assertions
        assert!(ans.is_some(), "Expected Some(Ide), but got None");

        let ide = ans.unwrap();
        assert_eq!(ide.name, "VS Code", "IDE name mismatch");
        assert_eq!(
            ide.working_directory.to_string_lossy(),
            "/home/foo/RustroverProjects/code-forge",
            "Working directory mismatch"
        );
        assert!(
            ide.workspace_id.as_str().contains("some_hash"),
            "Workspace ID should contain hash directory name"
        );
        assert!(ide.version.is_none(), "Version should be None");
    }

    #[tokio::test]
    async fn test_get_vscode_instance_different_project() {
        let dir = TempDir::new().unwrap();

        // Create User directory structure
        std::fs::create_dir_all(dir.path().join("User/globalStorage")).unwrap();
        std::fs::create_dir_all(dir.path().join("User/workspaceStorage/another_hash")).unwrap();

        // Create storage.json with a different project
        let storage_json = serde_json::json!({
            "windowsState": {
                "lastActiveWindow": {
                    "folder": "file:///home/foo/OtherProject"
                },
                "openedWindows": [
                    {
                        "folder": "file:///home/foo/OtherProject"
                    }
                ]
            }
        });
        std::fs::write(
            dir.path().join("User/globalStorage/storage.json"),
            serde_json::to_string_pretty(&storage_json).unwrap(),
        )
        .unwrap();

        // Create workspace.json for a different project
        let workspace_json = serde_json::json!({
            "folder": "file:///home/foo/OtherProject"
        });
        std::fs::write(
            dir.path()
                .join("User/workspaceStorage/another_hash/workspace.json"),
            serde_json::to_string_pretty(&workspace_json).unwrap(),
        )
        .unwrap();

        let cmd = vec![
            format!(
                "/usr/lib/electron32/electron --user-data-dir={}",
                dir.path().display()
            ),
            "--vscode-window-config".to_string(),
        ];

        let working_directory = "/home/foo/OtherProject".to_string();
        let cwd = "/home/foo/RustroverProjects/code-forge";
        let ans = get_vscode_instance(cmd, working_directory, cwd, 0).await;

        // Assertions
        assert!(ans.is_none(), "Expected None for a different project");
    }
}
