use std::collections::HashSet;

use forge_code::Code;
use forge_domain::{Ide, IdeRepository, Workspace};

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

    async fn get_workspace(&self) -> anyhow::Result<Workspace> {
        let mut combined_workspace = Workspace::default();
        let mut got_first = false;

        if let Ok(active_ides) = self.get_active_ides().await {
            for _ide in active_ides {
                match self.get_ide_workspace().await {
                    Ok(workspace) => {
                        if !got_first {
                            combined_workspace.workspace_id = workspace.workspace_id;
                            combined_workspace.focused_file = workspace.focused_file;
                            got_first = true;
                        }
                        combined_workspace.opened_files.extend(workspace.opened_files);
                    }
                    Err(_) => continue,
                }
            }
        }

        if !got_first {
            anyhow::bail!("No active IDEs found");
        }

        Ok(combined_workspace)
    }
}

impl Live {
    async fn get_ide_workspace(&self) -> anyhow::Result<Workspace> {
        for ide in &self.ides {
            if let Ok(workspace) = ide.get_workspace().await {
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

    async fn get_workspace(&self) -> anyhow::Result<Workspace> {
        match self {
            IdeType::VsCode(code) => code.get_workspace().await,
        }
    }
}