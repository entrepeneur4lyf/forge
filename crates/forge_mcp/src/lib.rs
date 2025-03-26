//! The `forge_mcp` crate implements the Model Context Protocol client functionality
//! for Forge, allowing it to connect to MCP servers, access resources, and invoke tools.

mod client;
mod error;
pub mod transport;
mod resources;
pub mod tools;

pub use client::MCPClient;
pub use error::{MCPError, Result};
pub use resources::{Resource, ResourceContent};
pub use tools::{MCPToolAdapter, ToolDefinition, ToolResult};

/// Represents server capabilities as reported during MCP initialization
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ServerCapabilities {
    /// Resources capability
    #[serde(default)]
    pub resources: Option<ResourcesCapability>,
    
    /// Tools capability
    #[serde(default)]
    pub tools: Option<ToolsCapability>,
    
    /// Prompts capability
    #[serde(default)]
    pub prompts: Option<PromptsCapability>,
    
    /// Sampling capability
    #[serde(default)]
    pub sampling: Option<SamplingCapability>,
}

/// Resources capability configuration
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ResourcesCapability {}

/// Tools capability configuration
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ToolsCapability {}

/// Prompts capability configuration
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct PromptsCapability {}

/// Sampling capability configuration
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct SamplingCapability {}