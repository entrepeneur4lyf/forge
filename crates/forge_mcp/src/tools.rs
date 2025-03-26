//! MCP tool types and adapters

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;

use forge_domain::{ExecutableTool, NamedTool, ToolDescription, ToolName};

use crate::MCPClient;

/// MCP tool definition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDefinition {
    /// Unique identifier for the tool
    pub name: String,
    
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// JSON Schema for the tool's parameters
    pub input_schema: Value,
    
    /// Optional JSON Schema for the tool's return value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

/// Result of a tool execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Whether the tool execution resulted in an error
    pub is_error: bool,
    
    /// The tool execution output
    pub content: String,
}

/// Adapter between MCP tools and Forge tools
pub struct MCPToolAdapter {
    /// The MCP client to use for tool execution
    client: Arc<MCPClient>,
    
    /// The MCP tool definition
    definition: ToolDefinition,
    
    /// Name to use for the tool (prefixed with mcp_)
    name: ToolName,
}

impl MCPToolAdapter {
    /// Create a new MCP tool adapter
    pub fn new(client: Arc<MCPClient>, definition: ToolDefinition) -> Self {
        let name = ToolName::new(format!("mcp_{}", definition.name));
        Self { client, definition, name }
    }
}

#[async_trait]
impl ExecutableTool for MCPToolAdapter {
    type Input = Value;

    async fn call(&self, input: Self::Input) -> anyhow::Result<String> {
        let result = self.client
            .call_tool(&self.definition.name, input)
            .await
            .map_err(|e| anyhow::anyhow!("MCP tool error: {}", e))?;
            
        if result.is_error {
            anyhow::bail!("Tool execution failed: {}", result.content);
        }
        
        Ok(result.content)
    }
}

impl ToolDescription for MCPToolAdapter {
    fn description(&self) -> String {
        format!(
            "MCP Tool: {}\n\n{}",
            self.definition.name,
            self.definition.description.clone().unwrap_or_default()
        )
    }
}

impl NamedTool for MCPToolAdapter {
    fn tool_name() -> ToolName {
        todo!("This will be provided via the instance method")
    }
}

// Custom implementation to get around the trait's static method
impl MCPToolAdapter {
    pub fn get_tool_name(&self) -> ToolName {
        self.name.clone()
    }
}