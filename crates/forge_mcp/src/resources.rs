//! MCP resource types

use serde::{Deserialize, Serialize};

/// Represents an MCP resource
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resource {
    /// Unique identifier for the resource
    pub uri: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Optional MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource content returned from a resource/read request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceContent {
    /// URI of the resource
    pub uri: String,
    
    /// MIME type (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    
    /// Text content (for text resources)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    
    /// Base64-encoded binary content (for binary resources)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

impl ResourceContent {
    /// Returns the resource content as text if available
    pub fn as_text(&self) -> Option<&str> {
        self.text.as_deref()
    }
    
    /// Returns the resource content as binary data if available
    pub fn as_binary(&self) -> Option<Vec<u8>> {
        self.blob.as_ref().and_then(|b| base64::decode(b).ok())
    }
    
    /// Returns true if this is a text resource
    pub fn is_text(&self) -> bool {
        self.text.is_some()
    }
    
    /// Returns true if this is a binary resource
    pub fn is_binary(&self) -> bool {
        self.blob.is_some()
    }
}

/// Type for resource URI templates
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceTemplate {
    /// URI template pattern
    pub uri_template: String,
    
    /// Human-readable name for this template
    pub name: String,
    
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Optional MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}