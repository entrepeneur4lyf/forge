#![allow(unused)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use forge_domain::{LoaderService, McpHttpServerConfig, McpService, RunnableService, Services, ToolDefinition, ToolName, VERSION};
use rmcp::model::{CallToolRequestParam, CallToolResult, ClientInfo, Implementation};
use rmcp::ServiceExt;
use serde_json::Value;
use tokio::sync::Mutex;
use crate::Infrastructure;
use crate::loader::ForgeLoaderService;

struct ServerHolder {
    client: Arc<RunnableService>,
    tool_definition: ToolDefinition,
    server_name: String,
}

/// Currently just a placeholder structure, to be implemented
/// when we add actual server functionality.
#[derive(Clone)]
pub struct ForgeMcp<F> {
    servers: Arc<Mutex<HashMap<ToolName, ServerHolder>>>,
    loader: ForgeLoaderService<F>,
}

impl<F: Infrastructure> ForgeMcp<F> {
    pub fn new(loader: ForgeLoaderService<F>) -> Self {
        Self { servers: Arc::new(Mutex::new(HashMap::new())), loader }
    }
    pub fn client_info() -> ClientInfo {
        ClientInfo {
            protocol_version: Default::default(),
            capabilities: Default::default(),
            client_info: Implementation { name: "Forge".to_string(), version: VERSION.to_string() },
        }
    }
    async fn start_http_server(
        &self,
        server_name: &str,
        config: McpHttpServerConfig,
    ) -> anyhow::Result<()> {
        let transport = rmcp::transport::SseTransport::start(config.url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start server: {e}"))?;

        let client = Self::client_info()
            .serve(transport)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to serve client: {e}"))?;

        let tools = client
            .list_tools(None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list tools: {e}"))?;
        let client = Arc::new(RunnableService::Http(client));

        let mut lock = self.servers.lock().await;
        for tool in tools.tools.into_iter() {
            let tool_name = ToolName::prefixed(hex::encode(server_name.as_bytes()), tool.name);
            lock.insert(
                tool_name.clone(),
                ServerHolder {
                    client: client.clone(),
                    tool_definition: ToolDefinition {
                        name: tool_name,
                        description: tool.description.unwrap_or_default().to_string(),
                        input_schema: serde_json::from_str(&serde_json::to_string(
                            &tool.input_schema,
                        )?)?,
                        output_schema: None,
                    },
                    server_name: server_name.to_string(),
                },
            );
        }
        drop(lock);

        Ok(())
    }
}

#[async_trait::async_trait]
impl<F: Infrastructure> McpService for ForgeMcp<F> {
    async fn init_mcp(&self) -> anyhow::Result<()> {
        let _ = self.stop_all_servers().await;
        match self.loader.load().await?.mcp {
            None => Ok(()),
            Some(config) => {
                if let Some(servers) = config.http {
                    let results: Vec<anyhow::Result<()>> = futures::future::join_all(
                        servers
                            .iter()
                            .map(|(server_name, server)| {
                                let server_config = server.clone();
                                async move {
                                    self.start_http_server(server_name, server_config).await?;
                                    Ok(())
                                }
                            })
                            .collect::<Vec<_>>(),
                    )
                        .await;
                    for i in results {
                        if let Err(e) = i {
                            tracing::error!("Failed to start server: {e}");
                        }
                    }
                }

                Ok(())
            }
        }
    }
    async fn list_tools(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        self.servers
            .lock()
            .await
            .iter()
            .map(|(_, server)| Ok(server.tool_definition.clone()))
            .collect()
    }

    async fn stop_all_servers(&self) -> anyhow::Result<()> {
        let mut servers = self.servers.lock().await;
        for (name, server) in servers.drain() {
            Arc::into_inner(server.client)
                .context(anyhow::anyhow!("Failed to stop server {name}"))?
                .cancel()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to stop server {name}: {e}"))?;
        }
        servers.clear();
        Ok(())
    }

    async fn get_service(&self, tool_name: &str) -> anyhow::Result<Arc<RunnableService>> {
        let servers = self.servers.lock().await;
        if let Some(server) = servers.get(&ToolName::new(tool_name)) {
            Ok(server.client.clone())
        } else {
            Err(anyhow::anyhow!("Server not found"))
        }
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> anyhow::Result<CallToolResult> {
        let tool_name = ToolName::new(tool_name);
        let servers = self.servers.lock().await;
        if let Some(server) = servers.get(&tool_name) {
            Ok(server
                .client
                .call_tool(CallToolRequestParam {
                    name: Cow::Owned(tool_name.striped_prefix()),
                    arguments: arguments.as_object().cloned(),
                })
                .await?)
        } else {
            Err(anyhow::anyhow!("Server not found"))
        }
    }
}
