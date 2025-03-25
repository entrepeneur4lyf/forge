use anyhow::{Context as _, Result};
use async_trait::async_trait;
use forge_domain::{ChatCompletionMessage, Content, ContentFull, Context as ForgeContext, Model, ModelId, Provider, ProviderService, ResultStream};
use reqwest::{Client, Url};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;
use url::Url as BaseUrl;

use crate::context::Context as MCPContext;

/// MCP client configuration
#[derive(Debug, Clone)]
pub struct MCPConfig {
    /// Base URL for MCP API
    pub url: BaseUrl,

    /// API key for authentication
    pub api_key: String,
}

/// Client for MCP service
#[derive(Clone)]
pub struct MCPClient {
    /// HTTP client for making requests
    client: Client,

    /// Configuration for MCP
    config: MCPConfig,
}

/// MCP Provider implementation
#[derive(Clone)]
pub struct MCPProvider {
    /// The underlying client
    client: Arc<MCPClient>,
}

impl MCPProvider {
    /// Creates a new MCP provider with the given client
    pub fn new(client: MCPClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

#[async_trait]
impl ProviderService for MCPProvider {
    async fn chat(
        &self,
        model: &ModelId,
        context: ForgeContext,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
        self.client.chat(model, context).await
    }

    async fn models(&self) -> Result<Vec<Model>> {
        self.client.models().await
    }
}

impl MCPClient {
    /// Creates a new MCP client with the given configuration
    pub fn new(config: MCPConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Creates a new MCP client from a Provider configuration
    pub fn from_provider(provider: Provider) -> Result<Self> {
        match provider {
            Provider::OpenAI { url, key } => {
                let key = key.ok_or_else(|| anyhow::anyhow!("API key is required for MCP provider"))?;
                Ok(Self::new(MCPConfig {
                    url,
                    api_key: key,
                }))
            }
            Provider::Anthropic { .. } => {
                anyhow::bail!("Anthropic provider is not supported for MCP")
            }
            Provider::MCP { url, key } => {
                anyhow::bail!("MCP provider is not supported for MCP");
            }
        }
    }

    /// Builds HTTP headers for requests
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.config.api_key)).unwrap(),
        );
        headers.insert("X-Title", HeaderValue::from_static("code-forge-mcp"));
        headers
    }

    /// Constructs a URL for the API endpoint
    fn url(&self, path: &str) -> anyhow::Result<Url> {
        // Validate the path doesn't contain certain patterns
        if path.contains("://") || path.contains("..") {
            anyhow::bail!("Invalid path: Contains forbidden patterns");
        }

        // Remove leading slash to avoid double slashes
        let path = path.trim_start_matches('/');

        self.config.url.join(path).with_context(|| {
            format!(
                "Failed to append {} to base URL: {}",
                path,
                self.config.url
            )
        })
    }

    /// Creates a new MCP context
    pub async fn create_context(&self, context: &MCPContext) -> Result<String> {
        let url = self.url("contexts")?;
        let response = self.client
            .post(url)
            .headers(self.headers())
            .json(&context)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let context_id = response["id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse context ID from response"))?
            .to_string();

        Ok(context_id)
    }

    /// Gets available MCP-capable models
    pub async fn models(&self) -> Result<Vec<Model>> {
        let url = self.url("models")?;
        debug!(url = %url, "Fetching MCP models");

        let response = self.client
            .get(url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Model>>()
            .await?;

        Ok(response)
    }

    /// Sends a chat request to the MCP API
    pub async fn chat(
        &self,
        model: &ModelId,
        context: ForgeContext,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
        let url = self.url("chat")?;
        debug!(url = %url, "Sending MCP chat request");

        // For now, we'll use a simplified implementation that delegates to a regular chat
        // In a real implementation, this would use MCP-specific endpoints
        let request = serde_json::json!({
            "model": model.as_str(),
            "messages": context.messages,
            "stream": true,
            "tools": context.tools,
            "tool_choice": context.tool_choice,
        });

        // This is a placeholder. In a real implementation, we would stream the response
        // Similar to the OpenRouter provider implementation
        let response = self.client
            .post(url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        // This is a simplified implementation - in reality, we would stream responses
        let message = ChatCompletionMessage::assistant(Content::Full(ContentFull::from(response["content"].as_str()
            .unwrap_or("No content received from MCP API"))));

        Ok(Box::pin(tokio_stream::once(Ok(message))))
    }
}