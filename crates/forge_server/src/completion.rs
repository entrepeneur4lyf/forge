use std::path::PathBuf;

use forge_walker::Walker;
use serde::Serialize;

use crate::Result;

#[derive(Serialize)]
pub struct File {
    pub path: String,
    pub is_dir: bool,
}

pub struct Completion {
    path: PathBuf,
}

impl Completion {
    pub fn new(path: impl Into<String>) -> Self {
        // Store instance of PathBuf over a string.
        // As Sting could be relative.

        // TODO: need better error handling
        let path = PathBuf::from(path.into())
            .canonicalize()
            .expect("Failed to canonicalize path");
        Self { path }
    }

    pub async fn list(&self) -> Result<Vec<File>> {
        // Use the current working directory
        let walker = Walker::new(self.path.clone());

        let files = walker.get().await?;
        Ok(files
            .into_iter()
            .map(|file| File { path: file.path, is_dir: file.is_dir })
            .collect())
    }
}
