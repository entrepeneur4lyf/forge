//! MCP client implementation


use serde_json::{json, Value};
use crate::{
    error::{MCPError, Result},
    resources::{Resource, ResourceContent},
    ServerCapabilities,
    tools::{ToolDefinition, ToolResult},
    transport::Transport,
};

/// MCP client for connecting to and communicating with MCP servers
pub struct MCPClient {
    transport: Box<dyn Transport>,
    server_capabilities: ServerCapabilities,
}

impl MCPClient {
    /// Connect to an MCP server using the provided transport
    pub async fn connect(transport: Box<dyn Transport>) -> Result<Self> {
        // Initialize connection
        let initialize_result = transport.send_request(
            "initialize", 
            json!({
                "protocol": {
                    "version": "0.2.0"
                },
                "client": {
                    "name": "forge",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "resources": {},
                    "tools": {},
                    "prompts": {},
                    "sampling": {}
                }
            })
        ).await?;
        
        // Parse server capabilities
        let server_capabilities = initialize_result
            .get("capabilities")
            .ok_or(MCPError::ProtocolError("Missing capabilities in initialize response".to_string()))?;
        
        let server_capabilities: ServerCapabilities = serde_json::from_value(server_capabilities.clone())
            .map_err(|e| MCPError::ParseError(format!("Failed to parse server capabilities: {}", e)))?;
        
        // Send initialized notification
        transport.send_notification("initialized", json!({})).await?;
        
        Ok(Self { 
            transport,
            server_capabilities,
        })
    }
    
    /// Get the server's reported capabilities
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.server_capabilities
    }
    
    /// List available resources from the server
    pub async fn list_resources(&self) -> Result<Vec<Resource>> {
        if self.server_capabilities.resources.is_none() {
            return Err(MCPError::UnsupportedOperation("Server does not support resources".to_string()));
        }
        
        let result = self.transport.send_request("resources/list", json!({})).await?;
        
        let resources = result
            .get("resources")
            .ok_or(MCPError::ProtocolError("Missing resources in list_resources response".to_string()))?;
        
        let resources: Vec<Resource> = serde_json::from_value(resources.clone())
            .map_err(|e| MCPError::ParseError(format!("Failed to parse resources: {}", e)))?;
            
        Ok(resources)
    }
    
    /// Read a resource by its URI
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContent>> {
        if self.server_capabilities.resources.is_none() {
            return Err(MCPError::UnsupportedOperation("Server does not support resources".to_string()));
        }
        
        let result = self.transport.send_request(
            "resources/read", 
            json!({
                "uri": uri
            })
        ).await?;
        
        let contents = result
            .get("contents")
            .ok_or(MCPError::ProtocolError("Missing contents in read_resource response".to_string()))?;
        
        let contents: Vec<ResourceContent> = serde_json::from_value(contents.clone())
            .map_err(|e| MCPError::ParseError(format!("Failed to parse resource contents: {}", e)))?;
            
        Ok(contents)
    }
    
    /// Subscribe to resource updates
    pub async fn subscribe_resource(&self, uri: &str) -> Result<()> {
        if self.server_capabilities.resources.is_none() {
            return Err(MCPError::UnsupportedOperation("Server does not support resources".to_string()));
        }
        
        self.transport.send_request(
            "resources/subscribe", 
            json!({
                "uri": uri
            })
        ).await?;
        
        Ok(())
    }
    
    /// Unsubscribe from resource updates
    pub async fn unsubscribe_resource(&self, uri: &str) -> Result<()> {
        if self.server_capabilities.resources.is_none() {
            return Err(MCPError::UnsupportedOperation("Server does not support resources".to_string()));
        }
        
        self.transport.send_request(
            "resources/unsubscribe", 
            json!({
                "uri": uri
            })
        ).await?;
        
        Ok(())
    }
    
    /// List available tools from the server
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>> {
        if self.server_capabilities.tools.is_none() {
            return Err(MCPError::UnsupportedOperation("Server does not support tools".to_string()));
        }
        
        let result = self.transport.send_request("tools/list", json!({})).await?;
        
        let tools = result
            .get("tools")
            .ok_or(MCPError::ProtocolError("Missing tools in list_tools response".to_string()))?;
        
        let tools: Vec<ToolDefinition> = serde_json::from_value(tools.clone())
            .map_err(|e| MCPError::ParseError(format!("Failed to parse tools: {}", e)))?;
            
        Ok(tools)
    }
    
    /// Call a tool by name with the given arguments
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<ToolResult> {
        if self.server_capabilities.tools.is_none() {
            return Err(MCPError::UnsupportedOperation("Server does not support tools".to_string()));
        }
        
        let result = self.transport.send_request(
            "tools/call", 
            json!({
                "name": name,
                "arguments": args
            })
        ).await?;
        
        let content = result
            .get("content")
            .ok_or(MCPError::ProtocolError("Missing content in call_tool response".to_string()))?;
            
        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let content: Vec<Value> = serde_json::from_value(content.clone())
            .map_err(|e| MCPError::ParseError(format!("Failed to parse tool result: {}", e)))?;
            
        // Extract the text content from the result
        let text_content = content.iter()
            .filter_map(|item| {
                if let Some(obj) = item.as_object() {
                    if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                        return obj.get("text").and_then(|t| t.as_str()).map(|s| s.to_string());
                    }
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(ToolResult {
            is_error,
            content: text_content,
        })
    }
    
    /// Close the connection to the server
    pub async fn close(self) -> Result<()> {
        self.transport.send_notification("exit", json!({})).await
    }
}