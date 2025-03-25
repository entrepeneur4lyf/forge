use anyhow::Result;
use forge_domain::{ChatCompletionMessage, Model, ModelId};
use serde::{Deserialize, Serialize};

use crate::context::{Context, ContextManager};

/// Manages context switching between multiple MCP contexts
#[derive(Clone, Debug)]
pub struct ContextSwitcher {
    /// The context manager
    context_manager: ContextManager,
    
    /// Whether context switching is enabled
    enabled: bool,
}

/// Represents a context switch action
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SwitchAction {
    /// Switch to a different context by ID
    SwitchTo(String),
    
    /// Create a new context and switch to it
    CreateAndSwitch { 
        /// The ID for the new context
        id: String, 
        
        /// The name for the new context
        name: String,
        
        /// The model ID to use for the new context
        model_id: Option<ModelId>,
        
        /// System message for the new context
        system_message: Option<String>,
    },
    
    /// Merge contexts and switch to the result
    MergeAndSwitch {
        /// IDs of contexts to merge
        context_ids: Vec<String>,
        
        /// ID for the new merged context
        new_id: String,
        
        /// Name for the new merged context
        new_name: String,
    },
}

impl ContextSwitcher {
    /// Creates a new context switcher
    pub fn new() -> Self {
        Self {
            context_manager: ContextManager::new(),
            enabled: true,
        }
    }
    
    /// Creates a new context switcher with the given context manager
    pub fn with_context_manager(context_manager: ContextManager) -> Self {
        Self {
            context_manager,
            enabled: true,
        }
    }
    
    /// Enables or disables context switching
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Gets whether context switching is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Gets the context manager
    pub fn context_manager(&self) -> &ContextManager {
        &self.context_manager
    }
    
    /// Gets a mutable reference to the context manager
    pub fn context_manager_mut(&mut self) -> &mut ContextManager {
        &mut self.context_manager
    }
    
    /// Performs a context switch action
    pub fn switch(&mut self, action: SwitchAction) -> Result<()> {
        if !self.enabled {
            anyhow::bail!("Context switching is disabled");
        }
        
        match action {
            SwitchAction::SwitchTo(id) => {
                self.context_manager.set_active_context(id)?;
            }
            
            SwitchAction::CreateAndSwitch { id, name, model_id, system_message } => {
                let mut context = Context::new(id.clone(), name);
                
                if let Some(model_id) = model_id {
                    context = context.with_model_id(model_id);
                }
                
                if let Some(system_message) = system_message {
                    context = context.with_system_message(system_message);
                }
                
                self.context_manager.add_context(context)?;
                self.context_manager.set_active_context(id)?;
            }
            
            SwitchAction::MergeAndSwitch { context_ids, new_id, new_name } => {
                if context_ids.len() < 2 {
                    anyhow::bail!("Need at least two contexts to merge");
                }
                
                // Start with the first two contexts
                self.context_manager.merge_contexts(
                    &context_ids[0], 
                    &context_ids[1], 
                    new_id.clone(), 
                    new_name
                )?;
                
                // Merge any additional contexts
                for i in 2..context_ids.len() {
                    let mut merged = self.context_manager.get_context(&new_id)
                        .ok_or_else(|| anyhow::anyhow!("Merged context not found"))?
                        .clone();
                    
                    let other = self.context_manager.get_context(&context_ids[i])
                        .ok_or_else(|| anyhow::anyhow!("Context with ID {} not found", context_ids[i]))?;
                    
                    merged.merge(other)?;
                    self.context_manager.remove_context(&new_id)?;
                    self.context_manager.add_context(merged)?;
                }
                
                self.context_manager.set_active_context(new_id)?;
            }
        }
        
        Ok(())
    }
    
    /// Detects if a message contains a context switch command
    pub fn detect_switch_command(&self, message: &ChatCompletionMessage) -> Option<SwitchAction> {
        if !self.enabled {
            return None;
        }
        
        // Extract content from message
        let content = message.content.as_ref()?.as_str();
        
        // Simple parsing of switch commands
        // In a real implementation, this would be more robust
        
        // Check for switch to command
        if let Some(id) = content.strip_prefix("/switch ") {
            return Some(SwitchAction::SwitchTo(id.trim().to_string()));
        }
        
        // Check for create command
        if let Some(args) = content.strip_prefix("/create ") {
            let parts: Vec<&str> = args.splitn(3, " ").collect();
            if parts.len() >= 2 {
                return Some(SwitchAction::CreateAndSwitch {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    model_id: None,
                    system_message: parts.get(2).map(|&s| s.to_string()),
                });
            }
        }
        
        // Check for merge command
        if let Some(args) = content.strip_prefix("/merge ") {
            let parts: Vec<&str> = args.splitn(3, " ").collect();
            if parts.len() >= 3 {
                let context_ids: Vec<String> = parts[0].split(",").map(|s| s.trim().to_string()).collect();
                return Some(SwitchAction::MergeAndSwitch {
                    context_ids,
                    new_id: parts[1].to_string(),
                    new_name: parts[2].to_string(),
                });
            }
        }
        
        None
    }
}