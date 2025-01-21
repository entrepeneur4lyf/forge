use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use forge_domain::{ChatRequest, IdeRepository};
use forge_prompt::Prompt;
use handlebars::Handlebars;
use serde::Serialize;

use super::file_read::FileReadService;
use super::{PromptService, Service};

impl Service {
    pub fn user_prompt_service(
        file_read: Arc<dyn FileReadService>,
        all_ides: Arc<dyn IdeRepository>,
    ) -> impl PromptService {
        Live { file_read, all_ides }
    }
}

struct Live {
    file_read: Arc<dyn FileReadService>,
    all_ides: Arc<dyn IdeRepository>,
}

#[derive(Serialize)]
struct Context {
    task: String,
    files: Vec<FileRead>,
    focused_files: Vec<String>,
    opened_files: Vec<String>,
}

#[derive(Serialize)]
struct FileRead {
    path: String,
    content: String,
}

#[async_trait::async_trait]
impl PromptService for Live {
    async fn get(&self, request: &ChatRequest) -> Result<String> {
        let template = include_str!("../prompts/coding/user_task.md");
        let parsed_task = Prompt::parse(request.content.to_string());

        let mut file_contents = vec![];
        for file_path in parsed_task.files() {
            let content = self.file_read.read(file_path.clone().into()).await?;
            file_contents.push(FileRead { path: file_path, content });
        }

        let mut focused_files = vec![];
        let mut opened_files = vec![];

        if let Ok(ides) = self.all_ides.get_active_ides().await {
            for ide in ides {
                if let Ok(workspace) = self.all_ides.get_workspace(&ide.workspace_id).await {
                    opened_files.push(Self::opened_files_xml(&workspace.opened_files, &ide.name));
                    focused_files.push(Self::focused_file_xml(
                        workspace.focused_file.to_string_lossy(),
                        &ide.name,
                    ));
                }
            }
        }

        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);
        hb.register_escape_fn(|str| str.to_string());

        let ctx = Context {
            task: request.content.to_string(),
            files: file_contents,
            focused_files,
            opened_files,
        };

        Ok(hb.render_template(template, &ctx)?)
    }
}

impl Live {
    pub fn opened_files_xml(opened_files: &HashSet<PathBuf>, ide: &str) -> String {
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

    pub fn focused_file_xml<T: AsRef<str>>(focused_file: T, ide: &str) -> String {
        Self::enclose_in_xml_tag(ide, focused_file.as_ref())
    }
}

#[cfg(test)]
pub mod tests {

    use async_trait::async_trait;
    use forge_domain::{Ide, Workspace, WorkspaceId};

    use super::*;
    use crate::service::file_read::tests::TestFileReadService;

    struct MockIdeRepository;

    #[async_trait]
    impl IdeRepository for MockIdeRepository {
        async fn get_active_ides(&self) -> Result<Vec<Ide>> {
            Ok(vec![])
        }

        async fn get_workspace(&self, _: &WorkspaceId) -> Result<Workspace> {
            Ok(Workspace::default())
        }
    }

    #[tokio::test]
    async fn test_render_user_prompt() {
        let file_read = Arc::new(
            TestFileReadService::default()
                .add("foo.txt", "Hello World - Foo")
                .add("bar.txt", "Hello World - Bar"),
        );

        let request = ChatRequest::new(
            forge_domain::ModelId::new("gpt-3.5-turbo"),
            "read this file content from @foo.txt and @bar.txt",
        );
        let rendered_prompt = Service::user_prompt_service(file_read, Arc::new(MockIdeRepository))
            .get(&request)
            .await
            .unwrap();
        insta::assert_snapshot!(rendered_prompt);
    }
}
