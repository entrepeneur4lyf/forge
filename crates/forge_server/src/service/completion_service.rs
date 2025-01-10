use std::path::PathBuf;

use forge_domain::Cwd;
use forge_walker::Walker;
use serde::Serialize;

use super::Service;
use crate::Result;

#[derive(Serialize)]
pub struct File {
    pub path: String,
    pub is_dir: bool,
}

#[async_trait::async_trait]
pub trait CompletionService: Send + Sync {
    async fn list(&self) -> Result<Vec<File>>;
}

struct Live {
    path: Cwd,
}

impl Live {
    pub fn new(path: impl Into<Cwd>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait::async_trait]
impl CompletionService for Live {
    async fn list(&self) -> Result<Vec<File>> {
        let cwd = PathBuf::from(self.path.as_str()); // Use the current working directory
        let walker = Walker::new(cwd);

        let files = walker.get().await?;
        Ok(files
            .into_iter()
            .map(|file| File { path: file.path, is_dir: file.is_dir })
            .collect())
    }
}

impl Service {
    pub fn completion_service(path: impl Into<Cwd>) -> impl CompletionService {
        Live::new(path)
    }
}
