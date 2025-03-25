use anyhow::Result;
use forge_domain::{
    ChatCompletionMessage, Context as ForgeContext, ContextMessage,
    Model, ModelId, ToolCallFull, ToolChoice, ToolName,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a context in Multi-Context Programming
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Context {
    /// Unique identifier for this context
    pub id: String,
    
    /// Name of the context for user reference
    pub name: String,
    
    /// The Forge context containing messages and state
    pub context: ForgeContext,
    
    /// The associated model for this context
    pub model_id: Option<ModelId>,
    
    /// Metadata for this context
    pub metadata: HashMap<String, String>,
}

impl Context {
    /// Creates a new MCP context with the given ID and name
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            context: ForgeContext::default(),
            model_id: None,
            metadata: HashMap::new(),
        }
    }
    
    /// Adds a system message to this context
    pub fn with_system_message(mut self, content: impl Into<String>) -> Self {
        self.context = self.context.add_message(ContextMessage::system(content.into()));
        self
    }
    
    /// Adds a user message to this context
    pub fn with_user_message(mut self, content: impl Into<String>) -> Self {
        self.context = self.context.add_message(ContextMessage::user(content.into()));
        self
    }
    
    /// Sets the model ID for this context
    pub fn with_model_id(mut self, model_id: ModelId) -> Self {
        self.model_id = Some(model_id);
        self
    }
    
    /// Adds a key-value pair to the metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Adds an assistant message to this context
    pub fn add_assistant_message(
        &mut self, 
        content: impl Into<String>, 
        tool_calls: Option<Vec<ToolCallFull>>
    ) {
        self.context.messages.push(ContextMessage::assistant(content.into(), tool_calls));
    }
    
    /// Merges another context into this one
    pub fn merge(&mut self, other: &Context) -> Result<()> {
        // Add all messages from the other context
        for message in other.context.messages.iter() {
            self.context.messages.push(message.clone());
        }
        
        // Merge metadata
        for (key, value) in &other.metadata {
            self.metadata.insert(key.clone(), value.clone());
        }
        
        Ok(())
    }
}

/// Manages multiple contexts for MCP
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ContextManager {
    /// Map of context ID to Context
    contexts: HashMap<String, Context>,
    
    /// Currently active context ID
    active_context_id: Option<String>,
}

impl ContextManager {
    /// Creates a new context manager
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            active_context_id: None,
        }
    }
    
    /// Adds a context to the manager
    pub fn add_context(&mut self, context: Context) -> Result<()> {
        let id = context.id.clone();
        self.contexts.insert(id.clone(), context);
        
        // If this is the first context, make it active
        if self.active_context_id.is_none() {
            self.active_context_id = Some(id);
        }
        
        Ok(())
    }
    
    /// Sets the active context by ID
    pub fn set_active_context(&mut self, id: impl Into<String>) -> Result<()> {
        let id = id.into();
        if !self.contexts.contains_key(&id) {
            anyhow::bail!("Context with ID {} does not exist", id);
        }
        
        self.active_context_id = Some(id);
        Ok(())
    }
    
    /// Gets the active context
    pub fn active_context(&self) -> Option<&Context> {
        self.active_context_id.as_ref().and_then(|id| self.contexts.get(id))
    }
    
    /// Gets a mutable reference to the active context
    pub fn active_context_mut(&mut self) -> Option<&mut Context> {
        let id = self.active_context_id.clone();
        id.and_then(move |id| self.contexts.get_mut(&id))
    }
    
    /// Gets a context by ID
    pub fn get_context(&self, id: &str) -> Option<&Context> {
        self.contexts.get(id)
    }
    
    /// Gets a mutable reference to a context by ID
    pub fn get_context_mut(&mut self, id: &str) -> Option<&mut Context> {
        self.contexts.get_mut(id)
    }
    
    /// Lists all available contexts
    pub fn list_contexts(&self) -> Vec<&Context> {
        self.contexts.values().collect()
    }
    
    /// Merges two contexts by ID and creates a new context with the merged content
    pub fn merge_contexts(
        &mut self, 
        id1: &str, 
        id2: &str, 
        new_id: impl Into<String>, 
        new_name: impl Into<String>
    ) -> Result<()> {
        let context1 = self.get_context(id1).ok_or_else(|| anyhow::anyhow!("Context with ID {} not found", id1))?;
        let context2 = self.get_context(id2).ok_or_else(|| anyhow::anyhow!("Context with ID {} not found", id2))?;
        
        let mut merged = Context::new(new_id, new_name);
        
        // Clone context1 data
        merged.context = context1.context.clone();
        merged.model_id = context1.model_id.clone();
        merged.metadata = context1.metadata.clone();
        
        // Merge in context2
        merged.merge(context2)?;
        
        // Add the merged context
        self.add_context(merged)?;
        
        Ok(())
    }
    
    /// Removes a context by ID
    pub fn remove_context(&mut self, id: &str) -> Result<()> {
        if !self.contexts.contains_key(id) {
            anyhow::bail!("Context with ID {} does not exist", id);
        }
        
        self.contexts.remove(id);
        
        // If we removed the active context, set another one as active
        if self.active_context_id.as_deref() == Some(id) {
            self.active_context_id = self.contexts.keys().next().cloned();
        }
        
        Ok(())
    }
}