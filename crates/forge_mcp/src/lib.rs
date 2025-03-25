// Multi-Context Programming (MCP) implementation for Forge
pub mod context;
pub mod provider;
pub mod switching;
pub mod workflow;

pub use context::Context as MCPContext;
pub use provider::{MCPProvider, MCPClient};