use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use forge_domain::Workflow;

use crate::context::{Context, ContextManager};
use crate::switching::ContextSwitcher;

/// Configuration for MCP in a workflow
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MCPConfig {
    /// Whether MCP is enabled for this workflow
    pub enabled: bool,
    
    /// Default contexts to create at startup
    pub default_contexts: Vec<ContextConfig>,
    
    /// Context switching configuration
    pub switching: SwitchingConfig,
}

/// Configuration for a context
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContextConfig {
    /// Unique ID for the context
    pub id: String,
    
    /// Display name for the context
    pub name: String,
    
    /// System message for the context
    pub system_message: Option<String>,
    
    /// User message for the context
    pub user_message: Option<String>,
    
    /// Model ID for the context
    pub model_id: Option<String>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Configuration for context switching
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SwitchingConfig {
    /// Whether context switching is enabled
    pub enabled: bool,
    
    /// Whether to automatically detect switch commands
    pub auto_detect: bool,
}

impl Default for SwitchingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect: true,
        }
    }
}

/// Extension trait to add MCP functionality to workflows
pub trait WorkflowMCPExt {
    /// Gets the MCP configuration from the workflow
    fn mcp_config(&self) -> Option<MCPConfig>;
    
    /// Creates a context manager from the workflow configuration
    fn create_context_manager(&self) -> Result<ContextManager>;
    
    /// Creates a context switcher from the workflow configuration
    fn create_context_switcher(&self) -> Result<ContextSwitcher>;
}

impl WorkflowMCPExt for Workflow {
    fn mcp_config(&self) -> Option<MCPConfig> {
        // self.config_value("mcp").ok()
        None
    }
    
    fn create_context_manager(&self) -> Result<ContextManager> {
        let config = self.mcp_config().unwrap_or_default();
        let mut manager = ContextManager::new();
        
        // Create default contexts
        for ctx_config in config.default_contexts {
            let mut context = Context::new(ctx_config.id.clone(), ctx_config.name);
            
            if let Some(system_message) = ctx_config.system_message {
                context = context.with_system_message(system_message);
            }
            
            if let Some(user_message) = ctx_config.user_message {
                context = context.with_user_message(user_message);
            }
            
            if let Some(model_id) = ctx_config.model_id {
                context = context.with_model_id(
                    forge_domain::ModelId::new(model_id)
                );
            }
            
            // Add metadata
            for (key, value) in ctx_config.metadata {
                context = context.with_metadata(key, value);
            }
            
            manager.add_context(context)?;
        }
        
        Ok(manager)
    }
    
    fn create_context_switcher(&self) -> Result<ContextSwitcher> {
        let config = self.mcp_config().unwrap_or_default();
        let manager = self.create_context_manager()?;
        let mut switcher = ContextSwitcher::with_context_manager(manager);
        
        switcher.set_enabled(config.switching.enabled);
        
        Ok(switcher)
    }
}