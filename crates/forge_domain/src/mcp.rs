use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use crate::NamedTool;


// ref: https://modelcontextprotocol.io/docs/concepts/architecture

/// Extra contextual information passed to request handlers
pub struct RequestHandlerExtra {
    pub client_id: String,
    pub session_id: Option<String>,
    pub timestamp: std::time::SystemTime,
    pub trace_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Optional request configuration
#[derive(Default)]
pub struct RequestOptions {
    pub timeout_ms: Option<u64>,
    pub retries: Option<u8>,
    pub priority: Option<u8>, // e.g., 0 = low, 10 = high
    pub auth_token: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Standard JSON-RPC error codes
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[async_trait::async_trait]
pub trait Protocol {
    type Request: Send + Sync + 'static;
    type Notification: Send + Sync + 'static;
    type Result: Send + Sync + 'static;

    /// Register a handler for a specific schema type
    async fn set_request_handler<T>(
        &mut self,
        handler: Box<dyn Fn(Self::Request, RequestHandlerExtra) -> BoxFuture<'static, Self::Result> + Send + Sync>,
    )
        where
            T: NamedTool + Send + Sync + 'static;

    /// Register a notification handler
    async fn set_notification_handler<T>(
        &mut self,
        handler: Box<dyn Fn(Self::Notification) -> BoxFuture<'static, ()> + Send + Sync>,
    )
        where
            T: NamedTool + Send + Sync + 'static;

    /// Send a request and get a typed response
    async fn request<T: NamedTool + Send + Sync + 'static>(
        &self,
        request: Self::Request,
        schema: &T,
        options: Option<RequestOptions>,
    ) -> Result<Self::Result, ErrorCode>;

    /// Send a one-way notification
    async fn notification(&self, notification: Self::Notification);
}
