use std::collections::HashSet;

use forge_code::Code;
use forge_domain::{Ide, IdeRepository, Workspace, WorkspaceId};

use crate::Service;

struct Live {
    ides: Vec<IdeType>,
}

enum IdeType {
    VsCode(Code),
}

impl Live {
    fn new<T: ToString>(cwd: T) -> Self {
        let ides: Vec<IdeType> = vec![IdeType::VsCode(Code::new(cwd.to_string()))];
        Self { ides }
    }
}

impl Service {
    pub fn ide_service<T: ToString>(cwd: T) -> impl IdeRepository {
        Live::new(cwd)
    }
}

#[async_trait::async_trait]
impl IdeRepository for Live {
    async fn get_active_ides(&self) -> anyhow::Result<HashSet<Ide>> {
        let mut files = HashSet::new();
        for ide in &self.ides {
            if let Ok(ide_files) = ide.get_active_ides().await {
                files.extend(ide_files);
            }
        }

        Ok(files)
    }

    async fn get_workspace(&self, ide: &WorkspaceId) -> anyhow::Result<Workspace> {
        for i in &self.ides {
            if let Ok(workspace) = i.get_workspace(ide).await {
                return Ok(workspace);
            }
        }

        anyhow::bail!("IDE not found")
    }
}

#[async_trait::async_trait]
impl IdeRepository for IdeType {
    async fn get_active_ides(&self) -> anyhow::Result<HashSet<Ide>> {
        match self {
            IdeType::VsCode(ide) => ide.get_active_ides().await,
        }
    }

    async fn get_workspace(&self, ide: &WorkspaceId) -> anyhow::Result<Workspace> {
        match self {
            IdeType::VsCode(code) => code.get_workspace(ide).await,
        }
    }
}
