use std::collections::HashSet;
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::{Ide, IdeRepository, Workspace};

use crate::db::Db;
use crate::process::Process;

/// Represents Visual Studio Code IDE interaction
pub struct Code {
    cwd: String,
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

        Self { cwd }
    }
}

#[async_trait]
impl IdeRepository for Code {
    async fn get_active_ides(&self) -> anyhow::Result<HashSet<Ide>> {
        Process::new(&self.cwd).instances().await
    }

    async fn get_workspace(&self) -> anyhow::Result<Workspace> {
        let mut combined_workspace = Workspace::default();
        let mut got_first = false;

        // Get all active IDE instances
        if let Ok(instances) = self.get_active_ides().await {
            for ide in instances {
                if let Ok(workspace) = Db::new(ide.workspace_id)?.get_workspace().await {
                    if !got_first {
                        combined_workspace.workspace_id = workspace.workspace_id;
                        combined_workspace.focused_file = workspace.focused_file;
                        got_first = true;
                    }
                    combined_workspace.opened_files.extend(workspace.opened_files);
                }
            }
        }

        if !got_first {
            anyhow::bail!("No active VS Code instances found");
        }

        Ok(combined_workspace)
    }
}