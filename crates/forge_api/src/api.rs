use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use forge_app::{EnvironmentService, ForgeApp, Infrastructure};
use forge_domain::*;
use forge_infra::{ForgeInfra, Resolved};
use forge_snaps::{FileSnapshotService, SnapshotService};
use forge_stream::MpscStream;
use serde_json::Value;

use crate::executor::ForgeExecutorService;
use crate::loader::ForgeLoaderService;
use crate::suggestion::ForgeSuggestionService;
use crate::API;

pub struct ForgeAPI<F> {
    app: Arc<F>,
    executor_service: ForgeExecutorService<F>,
    suggestion_service: ForgeSuggestionService<F>,
    loader: ForgeLoaderService<F>,
    snap_service: Arc<SnapshotService>,
}

impl<F: App + Infrastructure> ForgeAPI<F> {
    pub fn new(app: Arc<F>, snap_service: Arc<SnapshotService>) -> Self {
        Self {
            app: app.clone(),
            executor_service: ForgeExecutorService::new(app.clone()),
            suggestion_service: ForgeSuggestionService::new(app.clone()),
            loader: ForgeLoaderService::new(app.clone()),
            snap_service,
        }
    }
}

impl ForgeAPI<ForgeApp<ForgeInfra<Resolved>>> {
    pub fn init(restricted: bool) -> Self {
        let infra = ForgeInfra::new(restricted);
        let snap_service = Arc::new(SnapshotService::new(
            infra.env().get_environment().snapshot_path(),
        ));

        let infra = Arc::new(infra.transform(snap_service.clone()));
        let app = Arc::new(ForgeApp::new(infra));
        ForgeAPI::new(app, snap_service)
    }
}

#[async_trait::async_trait]
impl<F: App + Infrastructure> API for ForgeAPI<F> {
    async fn suggestions(&self) -> Result<Vec<File>> {
        self.suggestion_service.suggestions().await
    }

    async fn tools(&self) -> Vec<ToolDefinition> {
        self.app.tool_service().list()
    }

    async fn models(&self) -> Result<Vec<Model>> {
        Ok(self.app.provider_service().models().await?)
    }

    async fn chat(
        &self,
        chat: ChatRequest,
    ) -> anyhow::Result<MpscStream<Result<AgentMessage<ChatResponse>, anyhow::Error>>> {
        Ok(self.executor_service.chat(chat).await?)
    }

    async fn init(&self, workflow: Workflow) -> anyhow::Result<ConversationId> {
        self.app.conversation_service().create(workflow).await
    }

    fn environment(&self) -> Environment {
        self.app.environment_service().get_environment().clone()
    }

    async fn load(&self, path: Option<&Path>) -> anyhow::Result<Workflow> {
        self.loader.load(path).await
    }

    async fn conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> anyhow::Result<Option<Conversation>> {
        self.app.conversation_service().get(conversation_id).await
    }

    async fn get_variable(
        &self,
        conversation_id: &ConversationId,
        key: &str,
    ) -> anyhow::Result<Option<Value>> {
        self.app
            .conversation_service()
            .get_variable(conversation_id, key)
            .await
    }

    async fn set_variable(
        &self,
        conversation_id: &ConversationId,
        key: String,
        value: Value,
    ) -> anyhow::Result<()> {
        self.app
            .conversation_service()
            .set_variable(conversation_id, key, value)
            .await
    }

    fn snap_service(&self) -> Arc<dyn FileSnapshotService> {
        self.snap_service.clone()
    }
}
