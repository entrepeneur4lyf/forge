use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Context;
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
    pub fn new<T: ToString>(cwd: T) -> anyhow::Result<Self> {
        let cwd = cwd.to_string();

        // VS code stores the path without any trailing slashes.
        // We need to canonicalize the path to remove any trailing slashes.
        let cwd = PathBuf::from(&cwd)
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", cwd))?
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or(cwd);

        Ok(Self { cwd })
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
                // Clone workspace_id for use in error messages
                let workspace_id = ide.workspace_id.clone();
                let db = Db::new(ide.workspace_id)
                    .with_context(|| format!("Failed to create database for workspace: {:?}", workspace_id))?;
                if let Ok(workspace) = db.get_workspace().await
                    .with_context(|| format!("Failed to get workspace data for: {:?}", workspace_id)) {
                    if !got_first {
                        combined_workspace.workspace_id = workspace.workspace_id;
                        combined_workspace.focused_file = workspace.focused_file;
                        got_first = true;
                    }
                    combined_workspace
                        .opened_files
                        .extend(workspace.opened_files);
                }
            }
        }

        if !got_first {
            anyhow::bail!("No active VS Code instances found with accessible workspace data");
        }

        Ok(combined_workspace)
    }
}
