use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use forge_domain::Ide;
use forge_walker::Walker;
use serde_json::Value;
use sysinfo::System;

pub struct Process<'a> {
    cwd: &'a str,
}

impl<'a> Process<'a> {
    pub fn new(cwd: &'a str) -> Self {
        Self { cwd }
    }
    pub async fn instances(&'a self) -> anyhow::Result<HashSet<Ide>> {
        let mut ans = HashSet::new();
        let mut system = System::new_all();
        system.refresh_all();

        let processes = system
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
            .enumerate();

        for (i, process) in processes {
            let cmd = process
                .cmd()
                .iter()
                .map(|v| v.to_string_lossy().to_string())
                .collect::<Vec<_>>();

            let working_directory = process
                .cwd()
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .to_string();

            if let Some(ide) = get_instance(cmd, working_directory, self.cwd, i).await {
                ans.insert(ide);
            }
        }

        Ok(ans)
    }
}

async fn get_instance(
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

async fn extract_workspace_id(args: &[String], cwd: &str, index: usize) -> anyhow::Result<String> {
    let code_dir =
        extract_storage_dir(args).ok_or(anyhow!("Failed to extract storage directory"))?;
    let storage_file = PathBuf::from(format!("{}/User/globalStorage/storage.json", code_dir));
    let storage_json = tokio::fs::read_to_string(storage_file).await?;
    let json: Value = serde_json::from_str(&storage_json)?;
    let path_buf = PathBuf::from(code_dir.clone())
        .join("User")
        .join("workspaceStorage");
    let cwd = convert_path(cwd);

    // Not sure if matching index is good idea.
    let search_dir = if check_search_dir_condition(&json, &cwd, index) {
        cwd
    } else {
        return Err(anyhow!("Project not active in VS code"));
    };

    let hash_file = get_hash(Walker::new(path_buf.clone()), &search_dir, path_buf)
        .await
        .with_context(|| {
            format!(
                "Failed to locate workspace hash directory for: {}",
                search_dir
            )
        })?;

    Ok(hash_file.path)
}
fn extract_storage_dir(args: &[String]) -> Option<String> {
    args.iter()
        .find_map(|v| find_arg_value(&[v.clone()], "--user-data-dir="))
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

fn check_search_dir_condition(json_data: &Value, cwd: &str, index: usize) -> bool {
    // Check if the `openedWindows` array at the given index matches the `cwd`
    let opened_windows_query = format!("$.windowsState.openedWindows[{}].folder", index);

    let a = jsonpath_lib::select(json_data, &opened_windows_query)
        .ok()
        .and_then(|results| results.first().cloned())
        .and_then(|folder| folder.as_str())
        .and_then(|folder| folder.strip_prefix("file://").map(convert_path))
        .is_some_and(|path| path == cwd);

    // Check if the `lastActiveWindow.folder` matches the `cwd`
    let b = jsonpath_lib::select(json_data, "$.windowsState.lastActiveWindow.folder")
        .ok()
        .and_then(|results| results.first().cloned())
        .and_then(|folder| folder.as_str())
        .and_then(|folder| folder.strip_prefix("file://").map(convert_path))
        .is_some_and(|path| path == cwd);

    a || b
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

async fn process_workflow_file(path: &Path, cwd: &str) -> bool {
    let path = path.join("workspace.json");

    if let Ok(content) = tokio::fs::read_to_string(&path).await {
        let workflow_json: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse workspace JSON file: {}", path.display()))
            .unwrap_or_default();

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
}

#[cfg(test)]
mod partial_integration_tests {
    use tempfile::TempDir;

    use super::get_instance;

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
        let ans = get_instance(cmd, working_directory, cwd, 0).await;

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
        let ans = get_instance(cmd, working_directory, cwd, 0).await;

        // Assertions
        assert!(ans.is_none(), "Expected None for a different project");
    }
}
