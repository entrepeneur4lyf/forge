#![allow(unused)]

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::Context;
use forge_domain::{
    LoaderService, McpFsServerConfig, McpHttpServerConfig, McpService, RunnableService, Services,
    ToolDefinition, ToolName, VERSION,
};
use rmcp::model::{
    CallToolRequestParam, CallToolResult, ClientInfo, Implementation, ListToolsResult,
};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::loader::ForgeLoaderService;
use crate::Infrastructure;

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

    async fn insert_tools(
        &self,
        server_name: &str,
        tools: ListToolsResult,
        client: Arc<RunnableService>,
    ) -> anyhow::Result<()> {
        let mut lock = self.servers.lock().await;
        for tool in tools.tools.into_iter() {
            let tool_name = ToolName::prefixed(server_name, tool.name);
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

        Ok(())
    }

    async fn start_fs_server(
        &self,
        server_name: &str,
        config: McpFsServerConfig,
    ) -> anyhow::Result<()> {
        let mut command = Command::new(config.command);

        if let Some(env) = config.env {
            for (key, value) in env {
                command.env(key, value);
            }
        }

        let client = ().serve(TokioChildProcess::new(command.args(config.args))?).await?;
        let tools = client
            .list_tools(None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list tools: {e}"))?;
        let client = Arc::new(RunnableService::Fs(client));

        self.insert_tools(server_name, tools, client.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to insert tools: {e}"))?;

        Ok(())
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

        self.insert_tools(server_name, tools, client.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to insert tools: {e}"))?;

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
                if let Some(http_servers) = config.http {
                    let http_results: Vec<anyhow::Result<()>> = futures::future::join_all(
                        http_servers
                            .iter()
                            .map(|(server_name, server)| {
                                self.start_http_server(server_name, server.clone())
                            })
                            .collect::<Vec<_>>(),
                    )
                    .await;

                    for i in http_results {
                        if let Err(e) = i {
                            tracing::error!("Failed to start server: {e}");
                        }
                    }
                }

                if let Some(fs_servers) = config.fs {
                    let fs_results: Vec<anyhow::Result<()>> = futures::future::join_all(
                        fs_servers
                            .iter()
                            .map(|(server_name, server)| {
                                self.start_fs_server(server_name, server.clone())
                            })
                            .collect::<Vec<_>>(),
                    )
                    .await;

                    for i in fs_results {
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
        for (name, server_holder) in servers.drain() {
            // Get the Arc from the server holder
            let client = server_holder.client;

            // Log reference count info if high
            let ref_count = Arc::strong_count(&client);
            if ref_count > 1 {
                continue;
            }

            // Try to get exclusive ownership and call cancel
            if let Ok(service) = Arc::try_unwrap(client) {
                service
                    .cancel()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to stop server {name}: {e}"))?;
            }
        }
        servers.clear();
        Ok(())
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> anyhow::Result<CallToolResult> {
        let tool_name = ToolName::new(tool_name);
        let servers = self.servers.lock().await;
        if let Some(server) = servers.get(&tool_name) {
            Ok(server
                .client
                .call_tool(CallToolRequestParam {
                    name: Cow::Owned(tool_name.into_string()),
                    arguments: arguments.as_object().cloned(),
                })
                .await?)
        } else {
            Err(anyhow::anyhow!("Server not found"))
        }
    }
}
