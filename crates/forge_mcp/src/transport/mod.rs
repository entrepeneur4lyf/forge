//! Transport layer for MCP protocol

mod stdio;
mod http;

pub use stdio::StdioTransport;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::Result;

/// Transport interface for MCP communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a request to the MCP server and wait for a response
    async fn send_request(&self, method: &str, params: Value) -> Result<Value>;
    
    /// Send a notification to the MCP server (no response expected)
    async fn send_notification(&self, method: &str, params: Value) -> Result<()>;
}