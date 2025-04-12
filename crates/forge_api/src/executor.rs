use std::sync::Arc;

use forge_domain::{
    AgentMessage, ChatRequest, ChatResponse, ConversationService, Orchestrator, Services, Workflow,
};
use forge_stream::MpscStream;
use tracing::error;

pub struct ForgeExecutorService<F> {
    app: Arc<F>,
}
impl<F: Services> ForgeExecutorService<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self { app: infra }
    }
}

impl<F: Services> ForgeExecutorService<F> {
    pub async fn chat(
        &self,
        request: ChatRequest,
        workflow: Workflow,
    ) -> anyhow::Result<MpscStream<anyhow::Result<AgentMessage<ChatResponse>>>> {
        let app = self.app.clone();
        let conversation = app
            .conversation_service()
            .find(&request.conversation_id)
            .await?
            .expect("conversation for the request should've been created at this point.");
        Ok(MpscStream::spawn(move |tx| async move {
            let tx = Arc::new(tx);

            let orch = Orchestrator::new(app, conversation, Some(tx.clone()));

            if let Err(err) = orch.dispatch(request.event, &workflow).await {
                if let Err(e) = tx.send(Err(err)).await {
                    error!("Failed to send error to stream: {:#?}", e);
                }
            }
        }))
    }
}
