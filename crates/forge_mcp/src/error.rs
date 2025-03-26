//! Error types for MCP client

use thiserror::Error;

/// MCP client-specific error types
#[derive(Error, Debug)]
pub enum MCPError {
    /// Protocol-level error
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    /// Transport-layer error
    #[error("Transport error: {0}")]
    TransportError(String),
    
    /// Error parsing MCP response
    #[error("Parse error: {0}")]
    ParseError(String),
    
    /// Operation not supported by server
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
    
    /// Server returned an error
    #[error("Server error: {0} (code: {1})")]
    ServerError(String, i32),
    
    /// Other errors
    #[error("Error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for MCPError {
    fn from(err: serde_json::Error) -> Self {
        Self::ParseError(err.to_string())
    }
}

/// Result type for MCP operations
pub type Result<T> = std::result::Result<T, MCPError>;