use std::path::PathBuf;

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
    path: PathBuf,
}

impl Live {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait::async_trait]
impl CompletionService for Live {
    async fn list(&self) -> Result<Vec<File>> {
        let cwd = self.path.clone(); // Use the current working directory
        let walker = Walker::new(cwd);

        let files = walker.get().await?;
        Ok(files
            .into_iter()
            .map(|file| File { path: file.path, is_dir: file.is_dir })
            .collect())
    }
}

impl Service {
    pub fn completion_service(path: PathBuf) -> impl CompletionService {
        Live::new(path)
    }
}
