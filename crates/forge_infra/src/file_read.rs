use std::path::Path;

use anyhow::{Context, Result};
use bytes::Bytes;
use forge_app::FileService;

pub struct ForgeFileService;

impl Default for ForgeFileService {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeFileService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FileService for ForgeFileService {
    async fn read(&self, path: &Path) -> Result<Bytes> {
        Ok(tokio::fs::read(path)
            .await
            .map(Bytes::from)
            .with_context(|| format!("Failed to read file: {}", path.display()))?)
    }

    async fn write(&self, path: &Path, contents: Bytes) -> Result<()> {
        Ok(tokio::fs::write(path, contents)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?)
    }

    async fn create_dirs_all(&self, path: &Path) -> Result<()> {
        Ok(tokio::fs::create_dir_all(path)
            .await
            .with_context(|| format!("Failed to create dir: {}", path.display()))?)
    }
}
