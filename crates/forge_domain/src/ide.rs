use std::collections::HashSet;
use std::path::PathBuf;

use async_trait::async_trait;

/// Status of the current workspace in the IDE
#[derive(Debug, Default, derive_setters::Setters)]
pub struct Workspace {
    /// ID of the workspace
    pub workspace_id: WorkspaceId,

    /// List of open files in the IDE
    pub opened_files: HashSet<PathBuf>,

    /// The file that is currently focused in the IDE
    pub focused_file: PathBuf,
}

#[derive(Debug, Default, Clone, derive_more::From, PartialEq, Eq, Hash)]
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

pub struct IdeFilesInfo {
    pub opened_files: Vec<String>,
    pub focused_files: Vec<String>,
}

/// Represents functionality for interacting with IDEs
#[async_trait]
pub trait IdeRepository: Send + Sync {
    /// List of all the IDEs that are running on the system on the CWD.
    async fn get_active_ides(&self) -> anyhow::Result<HashSet<Ide>>;

    /// Get the status of workspace of the provided IDE
    async fn get_workspace(&self, ide: &WorkspaceId) -> anyhow::Result<Workspace>;
}

impl IdeFilesInfo {
    pub async fn from_ides(all_ides: &dyn IdeRepository) -> anyhow::Result<Self> {
        let mut focused_files = vec![];
        let mut opened_files = vec![];
        if let Ok(ides) = all_ides.get_active_ides().await {
            for ide in ides {
                if let Ok(workspace) = all_ides.get_workspace(&ide.workspace_id).await {
                    opened_files.push(Self::opened_files_xml(&workspace.opened_files, &ide.name));
                    focused_files.push(Self::focused_file_xml(
                        workspace.focused_file.to_string_lossy(),
                        &ide.name,
                    ));
                }
            }
        }

        Ok(Self { opened_files, focused_files })
    }
    fn opened_files_xml(opened_files: &HashSet<PathBuf>, ide: &str) -> String {
        opened_files
            .iter()
            .map(|f| f.to_string_lossy())
            .map(|v| Self::enclose_in_xml_tag(ide, v.as_ref()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn enclose_in_xml_tag(ide: &str, value: &str) -> String {
        let tag = match ide {
            "VS Code" => "vs_code_active_file",
            _ => "",
        };
        format!("<{}>{}</{}>", tag, value, tag)
    }

    fn focused_file_xml<T: AsRef<str>>(focused_file: T, ide: &str) -> String {
        Self::enclose_in_xml_tag(ide, focused_file.as_ref())
    }
}
