use std::sync::Arc;

use anyhow::Result;
use forge_domain::{ChatRequest, IdeRepository, Workspace};
use forge_prompt::Prompt;
use handlebars::Handlebars;
use serde::Serialize;

use super::file_read::FileReadService;
use super::{PromptService, Service};

impl Service {
    pub fn user_prompt_service(
        file: Arc<dyn FileReadService>,
        ide: Arc<dyn IdeRepository>,
    ) -> impl PromptService {
        Live { file, ide }
    }
}

struct Live {
    file: Arc<dyn FileReadService>,
    ide: Arc<dyn IdeRepository>,
}

#[derive(Serialize)]
struct Context {
    task: String,
    files: Vec<FileRead>,
    workspace: Workspace,
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
            let content = self.file.read(file_path.clone().into()).await?;
            file_contents.push(FileRead { path: file_path, content });
        }

        let workspace = self.ide.get_workspace().await?;

        let mut hb = Handlebars::new();
        hb.set_strict_mode(true);
        hb.register_escape_fn(|str| str.to_string());

        let ctx = Context {
            task: request.content.to_string(),
            files: file_contents,
            workspace,
        };

        Ok(hb.render_template(template, &ctx)?)
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashSet;

    use async_trait::async_trait;
    use forge_domain::{Ide, Workspace, WorkspaceId};

    use super::*;
    use crate::service::test::TestFileReadService;

    struct MockIdeRepository;

    #[async_trait]
    impl IdeRepository for MockIdeRepository {
        async fn get_active_ides(&self) -> Result<HashSet<Ide>> {
            Ok(Default::default())
        }

        async fn get_workspace(&self) -> Result<Workspace> {
            let mut workspace = Workspace::default();
            workspace.workspace_id = WorkspaceId::from("test-workspace-1".to_string());
            workspace.focused_file = "test.txt".to_string();
            workspace
                .opened_files
                .extend(["test.txt".to_string(), "other.txt".to_string()]);
            Ok(workspace)
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
