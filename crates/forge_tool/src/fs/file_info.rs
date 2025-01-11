use std::path::PathBuf;

use forge_domain::{Environment, ToolCallService, ToolDescription};
use forge_tool_macros::ToolDescription;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct FSFileInfoInput {
    /// The path of the file or directory to inspect (relative to the current
    /// working directory)
    pub path: String,
}

/// Request to retrieve detailed metadata about a file or directory at the
/// specified path. Returns comprehensive information including size, creation
/// time, last modified time, permissions, and type. Use this when you need to
/// understand file characteristics without reading the actual content.
#[derive(ToolDescription)]
pub struct FSFileInfo {
    environment: Environment,
}

impl FSFileInfo {
    pub fn new(environment: Environment) -> Self {
        Self { environment }
    }
}

#[async_trait::async_trait]
impl ToolCallService for FSFileInfo {
    type Input = FSFileInfoInput;
    type Output = String;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        let path = PathBuf::from(&input.path);

        // Validate the path before proceeding
        if !self.validate_path(&path, &self.environment).await? {
            return Err("Access to this path is not allowed".to_string());
        }

        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|e| e.to_string())?;
        Ok(format!("{:?}", meta))
    }
}

#[cfg(test)]
mod test {
    use tempfile::TempDir;
    use tokio::fs;

    use super::*;
    use crate::test_utils::setup_test_env;

    #[tokio::test]
    async fn test_fs_file_info_on_file() {
        let temp_dir = TempDir::new().unwrap();
        let environment = setup_test_env(&temp_dir).await;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test content").await.unwrap();

        let fs_info = FSFileInfo::new(environment);
        let result = fs_info
            .call(FSFileInfoInput { path: file_path.to_string_lossy().to_string() })
            .await
            .unwrap();

        assert!(result.contains("FileType"));
        assert!(result.contains("permissions"));
        assert!(result.contains("modified"));
    }

    #[tokio::test]
    async fn test_fs_file_info_on_directory() {
        let temp_dir = TempDir::new().unwrap();
        let environment = setup_test_env(&temp_dir).await;
        let dir_path = temp_dir.path().join("test_dir");
        fs::create_dir(&dir_path).await.unwrap();

        let fs_info = FSFileInfo::new(environment);
        let result = fs_info
            .call(FSFileInfoInput { path: dir_path.to_string_lossy().to_string() })
            .await
            .unwrap();

        assert!(result.contains("FileType"));
        assert!(result.contains("permissions"));
        assert!(result.contains("modified"));
    }

    #[tokio::test]
    async fn test_fs_file_info_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let environment = setup_test_env(&temp_dir).await;
        let nonexistent_path = temp_dir.path().join("nonexistent");

        let fs_info = FSFileInfo::new(environment);
        let result = fs_info
            .call(FSFileInfoInput { path: nonexistent_path.to_string_lossy().to_string() })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_file_info_hidden_file() {
        let temp_dir = TempDir::new().unwrap();
        let environment = setup_test_env(&temp_dir).await;
        let hidden_path = temp_dir.path().join(".hidden");
        fs::write(&hidden_path, "hidden content").await.unwrap();

        let fs_info = FSFileInfo::new(environment);
        let result = fs_info
            .call(FSFileInfoInput { path: hidden_path.to_string_lossy().to_string() })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_fs_file_info_gitignored_file() {
        let temp_dir = TempDir::new().unwrap();
        let environment = setup_test_env(&temp_dir).await;
        let ignored_path = temp_dir.path().join("ignored.txt");
        fs::write(&ignored_path, "ignored content").await.unwrap();

        let fs_info = FSFileInfo::new(environment);
        let result = fs_info
            .call(FSFileInfoInput { path: ignored_path.to_string_lossy().to_string() })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }
}
