use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use forge_domain::{Workspace, WorkspaceId};
use rusqlite::{Connection, OptionalExtension};

use crate::parse;

pub struct Db {
    pub conn: Connection,
    pub workspace_id: WorkspaceId,
}

impl Db {
    pub fn new(workspace_id: WorkspaceId) -> anyhow::Result<Self> {
        let db_path = PathBuf::from(workspace_id.as_str())
            .join("state.vscdb")
            .to_string_lossy()
            .to_string();

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open VS Code database at '{}'", db_path))?;

        Ok(Self { conn, workspace_id })
    }

    pub async fn get_workspace(self) -> anyhow::Result<Workspace> {
        let focused_file = self.extract_focused_file()?.to_string_lossy().to_string();
        let opened_files = self
            .extract_active_files()?
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        Ok(Workspace { workspace_id: self.workspace_id, focused_file, opened_files })
    }

    fn extract_focused_file(&self) -> anyhow::Result<PathBuf> {
        let key = "workbench.explorer.treeViewState";
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM ItemTable WHERE key = ?1")
            .with_context(|| format!("Failed to prepare SQL query for key '{}'", key))?;
        let value: Option<String> = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .optional()
            .with_context(|| format!("Failed to execute SQL query for key '{}'", key))?;

        if let Some(value) = value {
            return Ok(PathBuf::from(parse::focused_file_path(&value)?));
        }

        Err(anyhow!(
            "No active editor found - unable to determine focused file"
        ))
    }

    fn extract_active_files(&self) -> anyhow::Result<HashSet<PathBuf>> {
        let key = "memento/workbench.parts.editor";
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM ItemTable WHERE key = ?1")
            .with_context(|| format!("Failed to prepare SQL query for key '{}'", key))?;
        let value: Option<String> = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .optional()
            .with_context(|| format!("Failed to execute SQL query for key '{}'", key))?;

        if let Some(value) = value {
            return parse::active_files_path(&value);
        }

        Err(anyhow!("No active editor files found in workspace"))
    }
}
