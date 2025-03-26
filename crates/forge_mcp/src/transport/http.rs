//! HTTP with SSE transport for MCP protocol

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::{oneshot, Mutex};
use url::Url;

use crate::error::{MCPError, Result};
use super::Transport;

/// Transport implementation for HTTP+SSE based MCP servers
pub struct HttpTransport {
    /// Base URL of the MCP server
    base_url: Url,
    
    /// HTTP client
    http_client: reqwest::Client,
    
    /// Next request ID
    next_id: AtomicU64,
    
    /// Pending requests waiting for responses
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
}

impl HttpTransport {
    /// Create a new HTTP transport connecting to the given URL
    pub async fn new(base_url: Url) -> Result<Self> {
        let http_client = reqwest::Client::new();
        let transport = Self {
            base_url,
            http_client,
            next_id: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        };
        
        // Start SSE listener for server-to-client messages
        transport.start_sse_listener().await?;
        
        Ok(transport)
    }
    
    /// Start the SSE listener for receiving server-to-client messages
    async fn start_sse_listener(&self) -> Result<()> {
        // Implementation will depend on what SSE library is used
        // This would typically involve:
        // 1. Connecting to the SSE endpoint
        // 2. Parsing incoming events
        // 3. Matching responses to pending requests
        // 4. Handling server-initiated events
        
        // For now, return a placeholder error as this requires more dependencies
        Err(MCPError::TransportError(
            "HTTP+SSE transport is not yet implemented".to_string()
        ))
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        let (response_tx, response_rx) = oneshot::channel();
        
        // Store the response channel in pending requests
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, response_tx);
        }
        
        // Create the JSON-RPC request
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        
        // Send HTTP POST request to the server
        let url = self.base_url.join("rpc").map_err(|e| {
            MCPError::TransportError(format!("Invalid URL: {}", e))
        })?;
        
        let response = self.http_client.post(url)
            .json(&request)
            .send()
            .await
            .map_err(|e| MCPError::TransportError(format!("HTTP request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(MCPError::TransportError(
                format!("HTTP request failed with status: {}", response.status())
            ));
        }
        
        // In a real implementation, we would now wait for the response via SSE
        // For now, we'll just interpret the HTTP response directly
        let json_response: Value = response.json().await
            .map_err(|e| MCPError::ParseError(format!("Failed to parse response: {}", e)))?;
        
        // Clean up the pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&id);
        }
        
        // Parse the response
        if let Some(error) = json_response.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1) as i32;
            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            Err(MCPError::ServerError(message.to_string(), code))
        } else if let Some(result) = json_response.get("result") {
            Ok(result.clone())
        } else {
            Err(MCPError::ProtocolError("Invalid response format".to_string()))
        }
    }
    
    async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        // Create the JSON-RPC notification
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        
        // Send HTTP POST request to the server
        let url = self.base_url.join("rpc").map_err(|e| {
            MCPError::TransportError(format!("Invalid URL: {}", e))
        })?;
        
        let response = self.http_client.post(url)
            .json(&notification)
            .send()
            .await
            .map_err(|e| MCPError::TransportError(format!("HTTP request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(MCPError::TransportError(
                format!("HTTP notification failed with status: {}", response.status())
            ));
        }
        
        Ok(())
    }
}