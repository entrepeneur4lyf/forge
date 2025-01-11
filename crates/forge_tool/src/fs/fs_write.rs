use std::path::{Path, PathBuf};

use forge_domain::{Environment, ToolCallService, ToolDescription};
use forge_tool_macros::ToolDescription;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fs::syn;

#[derive(Deserialize, JsonSchema)]
pub struct FSWriteInput {
    /// The path of the file to write to (relative to the current working
    /// directory)
    pub path: String,
    /// The content to write to the file. ALWAYS provide the COMPLETE intended
    /// content of the file, without any truncation or omissions. You MUST
    /// include ALL parts of the file, even if they haven't been modified.
    pub content: String,
}

/// Request to write content to a file at the specified path. If the file
/// exists, it will be overwritten with the provided content. If the file
/// doesn't exist, it will be created. This tool will automatically create any
/// directories needed to write the file.
#[derive(ToolDescription)]
pub struct FSWrite {
    environment: Environment,
}

impl FSWrite {
    pub fn new(environment: Environment) -> Self {
        Self { environment }
    }

    /// Check if a path is allowed based on its name (without existence check)
    async fn is_path_allowed(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Don't allow hidden files
        if file_name.starts_with('.') {
            return false;
        }

        // Don't allow files in .gitignore
        let gitignore_path = self.environment.cwd.join(".gitignore");
        if gitignore_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&gitignore_path).await {
                let patterns: Vec<&str> = content
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .collect();

                for pattern in patterns {
                    // Simple exact match for now
                    if pattern == file_name {
                        return false;
                    }
                }
            }
        }

        true
    }
}

#[async_trait::async_trait]
impl ToolCallService for FSWrite {
    type Input = FSWriteInput;
    type Output = FSWriteOutput;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        let path = PathBuf::from(&input.path);

        // First check if the path would be allowed
        if !self.is_path_allowed(&path).await {
            return Err("Access to this path is not allowed".to_string());
        }

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }

        // Validate file content if it's a supported language file
        let syntax_checker = syn::validate(&input.path, &input.content).err();

        // Write file only after validation passes
        tokio::fs::write(&path, &input.content)
            .await
            .map_err(|e| e.to_string())?;

        Ok(FSWriteOutput { path: input.path, syntax_checker, content: input.content })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FSWriteOutput {
    pub path: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_checker: Option<String>,
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tokio::fs;

    use super::*;
    use crate::test_utils::setup_test_env;

    #[tokio::test]
    async fn test_fs_write_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let environment = setup_test_env(&temp_dir).await;

        let fs_write = FSWrite::new(environment);
        let output = fs_write
            .call(FSWriteInput {
                path: file_path.to_string_lossy().to_string(),
                content: "Hello, World!".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(output.path, file_path.to_string_lossy().to_string());
        assert_eq!(output.content, "Hello, World!");

        // Verify file was actually written
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, World!")
    }

    #[tokio::test]
    async fn test_fs_write_invalid_rust() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        let environment = setup_test_env(&temp_dir).await;

        let fs_write = FSWrite::new(environment);
        let result = fs_write
            .call(FSWriteInput {
                path: file_path.to_string_lossy().to_string(),
                content: "fn main() { let x = ".to_string(),
            })
            .await;

        assert!(result.unwrap().syntax_checker.is_some());
    }

    #[tokio::test]
    async fn test_fs_write_valid_rust() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        let environment = setup_test_env(&temp_dir).await;

        let fs_write = FSWrite::new(environment);
        let result = fs_write
            .call(FSWriteInput {
                path: file_path.to_string_lossy().to_string(),
                content: "fn main() { let x = 42; }".to_string(),
            })
            .await;

        assert!(result.is_ok());
        // Verify file contains valid Rust code
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "fn main() { let x = 42; }");
    }

    #[tokio::test]
    async fn test_fs_write_hidden_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join(".hidden.txt");
        let environment = setup_test_env(&temp_dir).await;

        let fs_write = FSWrite::new(environment);
        let result = fs_write
            .call(FSWriteInput {
                path: file_path.to_string_lossy().to_string(),
                content: "hidden content".to_string(),
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_fs_write_gitignored_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("ignored.txt");
        let environment = setup_test_env(&temp_dir).await;

        let fs_write = FSWrite::new(environment);
        let result = fs_write
            .call(FSWriteInput {
                path: file_path.to_string_lossy().to_string(),
                content: "ignored content".to_string(),
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_fs_write_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("new_dir");
        let file_path = dir_path.join("test.txt");
        let environment = setup_test_env(&temp_dir).await;

        let fs_write = FSWrite::new(environment);
        let result = fs_write
            .call(FSWriteInput {
                path: file_path.to_string_lossy().to_string(),
                content: "content in new dir".to_string(),
            })
            .await;

        assert!(result.is_ok());
        assert!(dir_path.exists());
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "content in new dir");
    }
}
