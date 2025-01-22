use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::anyhow;
use forge_domain::{Workspace, WorkspaceId};
use rusqlite::{Connection, OptionalExtension};

use crate::parse;

pub struct Db {
    pub conn: Connection,
    pub workspace_id: WorkspaceId,
}

impl Db {
    pub fn new(workspace_id: WorkspaceId) -> anyhow::Result<Self> {
        let conn = Connection::open(
            PathBuf::from(workspace_id.as_str())
                .join("state.vscdb")
                .to_string_lossy()
                .to_string(),
        )?;

        Ok(Self { conn, workspace_id })
    }

    pub async fn get_workspace(self) -> anyhow::Result<Workspace> {
        let mut ans = Workspace::default();

        ans = ans.focused_file(self.extract_focused_file()?);
        ans = ans.opened_files(self.extract_active_files()?);
        ans = ans.workspace_id(self.workspace_id);

        Ok(ans)
    }

    fn extract_focused_file(&self) -> anyhow::Result<PathBuf> {
        let key = "workbench.explorer.treeViewState";
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM ItemTable WHERE key = ?1")?;
        let value: Option<String> = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .optional()?;

        if let Some(value) = value {
            return Ok(PathBuf::from(parse::focused_file_path(&value)?));
        }

        Err(anyhow!("Focused file not found"))
    }

    fn extract_active_files(&self) -> anyhow::Result<HashSet<PathBuf>> {
        let key = "memento/workbench.parts.editor";
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM ItemTable WHERE key = ?1")?;
        let value: Option<String> = stmt
            .query_row(rusqlite::params![key], |row| row.get(0))
            .optional()?;

        if let Some(value) = value {
            return parse::active_files_path(&value);
        }

        Err(anyhow!("Focused file not found"))
    }
}
