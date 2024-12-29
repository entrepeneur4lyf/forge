use std::path::PathBuf;

use forge_tool_macros::Description as DescriptionDerive;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Description, ToolTrait};

#[derive(Deserialize, JsonSchema)]
pub struct FSWriteInput {
    #[schemars(
        description = "The path of the file to write to (relative to the current working directory {{cwd}})"
    )]
    pub path: String,
    #[schemars(
        description = "The content to write to the file. ALWAYS provide the COMPLETE intended content of the file, without any truncation or omissions. You MUST include ALL parts of the file, even if they haven't been modified."
    )]
    pub content: String,
}

/// Request to write content to a file at the specified path. If the file
/// exists, it will be overwritten with the provided content. If the file
/// doesn't exist, it will be created. This tool will automatically create any
/// directories needed to write the file. Parameters:
/// - path: (required) The path of the file to write to (relative to the current
///   working directory {{cwd}})
/// - content: (required) The content to write to the file. ALWAYS provide the
///   COMPLETE intended content of the file, without any truncation or
///   omissions. You MUST include ALL parts of the file, even if they haven't
///   been modified.
#[derive(DescriptionDerive, Default)]
pub struct FSWrite {
    parent: Option<String>,
}

impl FSWrite {
    pub fn new(parent: String) -> Self {
        Self { parent: Some(parent) }
    }
}

#[async_trait::async_trait]
impl ToolTrait for FSWrite {
    type Input = FSWriteInput;
    type Output = FSWriteOutput;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        let mut path = input.path;
        if let Some(parent) = &self.parent {
            path = PathBuf::from(parent)
                .join(path)
                .to_str()
                .ok_or("Invalid path".to_string())?
                .to_string();
        }

        tokio::fs::write(&path, &input.content)
            .await
            .map_err(|e| e.to_string())?;
        Ok(FSWriteOutput { path, content: input.content })
    }
}

#[derive(Serialize, JsonSchema)]
pub struct FSWriteOutput {
    pub path: String,
    pub content: String,
}

#[cfg(test)]
mod test {
    use tempfile::TempDir;
    use tokio::fs;

    use super::*;

    #[tokio::test]
    async fn test_fs_write_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let fs_write = FSWrite::default();
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
}
