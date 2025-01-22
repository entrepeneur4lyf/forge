use std::collections::HashSet;
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::{Ide, IdeRepository, Workspace, WorkspaceId};

use crate::db::Db;
use crate::process::Process;

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
    async fn get_active_ides(&self) -> anyhow::Result<HashSet<Ide>> {
        Process::new(&self.root_dir).instances().await
    }

    async fn get_workspace(&self, ide: &WorkspaceId) -> anyhow::Result<Workspace> {
        Db::new(ide.clone())?.get_workspace().await
    }
}
