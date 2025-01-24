use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;

/// Status of the current workspace in the IDE
#[derive(Debug, Default, derive_setters::Setters, Serialize, Clone)]
pub struct Workspace {
    /// ID of the workspace
    pub workspace_id: WorkspaceId,

    /// List of open files in the IDE
    pub opened_files: BTreeSet<String>,

    /// The file that is currently focused in the IDE
    pub focused_file: String,
}

#[derive(Debug, Default, Clone, derive_more::From, PartialEq, Eq, Hash, Serialize)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Represents an IDE. Contains meta information about the IDE.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Ide {
    pub name: String,
    pub version: Option<String>,
    pub working_directory: PathBuf,
    pub workspace_id: WorkspaceId,
}

/// Represents functionality for interacting with IDEs
#[async_trait]
pub trait IdeRepository: Send + Sync {
    /// List of all the IDEs that are running on the system on the CWD.
    async fn get_active_ides(&self) -> anyhow::Result<HashSet<Ide>>;

    /// Get the consolidated status of all active workspaces
    async fn get_workspace(&self) -> anyhow::Result<Workspace>;
}