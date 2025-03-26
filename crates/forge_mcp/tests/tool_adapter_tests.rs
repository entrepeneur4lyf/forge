//! Tests for MCP tool adapter functionality

#[cfg(test)]
mod tool_adapter_tests {
    use std::sync::Arc;
    
    use serde_json::json;
    
    use forge_domain::{ExecutableTool, ToolDescription};
    use forge_mcp::{Result, ToolDefinition, ToolResult};

    // For these tests, we need to use a trait object since we can't use the real MCPClient
    trait ToolCaller: Send + Sync {
        fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult>;
    }

    // Mock implementation that satisfies the trait
    struct MockToolCaller {
        // Whether calling tools should succeed or fail
        fail_calls: bool,
    }
    
    impl MockToolCaller {
        fn new(fail_calls: bool) -> Self {
            Self { fail_calls }
        }
    }
    
    impl ToolCaller for MockToolCaller {
        fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
            if self.fail_calls {
                return Ok(ToolResult {
                    is_error: true,
                    content: format!("Error calling tool {}: invalid arguments", name),
                });
            }
            
            // For the calculate_sum tool, actually calculate the sum
            if name == "calculate_sum" {
                if let (Some(a), Some(b)) = (
                    args.get("a").and_then(|v| v.as_i64()),
                    args.get("b").and_then(|v| v.as_i64()),
                ) {
                    return Ok(ToolResult {
                        is_error: false,
                        content: format!("{}", a + b),
                    });
                }
            }
            
            // Generic successful response for other tools
            Ok(ToolResult {
                is_error: false,
                content: format!("Called tool {} with args: {}", name, args),
            })
        }
    }

    // We'll implement a custom MCPToolAdapter for testing
    struct TestMCPToolAdapter {
        client: Arc<dyn ToolCaller>,
        definition: ToolDefinition,
        name: forge_domain::ToolName,
    }
    
    impl TestMCPToolAdapter {
        fn new(client: Arc<dyn ToolCaller>, definition: ToolDefinition) -> Self {
            let name = forge_domain::ToolName::new(format!("mcp_{}", definition.name));
            Self { client, definition, name }
        }
        
        fn get_tool_name(&self) -> forge_domain::ToolName {
            self.name.clone()
        }
    }
    
    #[async_trait::async_trait]
    impl ExecutableTool for TestMCPToolAdapter {
        type Input = serde_json::Value;

        async fn call(&self, input: Self::Input) -> anyhow::Result<String> {
            let result = self.client.call_tool(&self.definition.name, input)
                .map_err(|e| anyhow::anyhow!("MCP tool error: {}", e))?;
                
            if result.is_error {
                anyhow::bail!("Tool execution failed: {}", result.content);
            }
            
            Ok(result.content)
        }
    }
    
    impl ToolDescription for TestMCPToolAdapter {
        fn description(&self) -> String {
            format!(
                "MCP Tool: {}\n\n{}",
                self.definition.name,
                self.definition.description.clone().unwrap_or_default()
            )
        }
    }
    
    #[tokio::test]
    async fn test_tool_adapter_successful_call() {
        // Create a mock MCP client
        let client = Arc::new(MockToolCaller::new(false));
        
        // Create a tool definition
        let definition = ToolDefinition {
            name: "calculate_sum".to_string(),
            description: Some("Add two numbers together".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": { "type": "number" },
                    "b": { "type": "number" }
                },
                "required": ["a", "b"]
            }),
            output_schema: None,
        };
        
        // Create the adapter
        let adapter = TestMCPToolAdapter::new(client.clone(), definition);
        
        // Call the tool
        let result = adapter.call(json!({
            "a": 15,
            "b": 27
        })).await.expect("Failed to call tool");
        
        // Verify the result
        assert_eq!(result, "42");
        
        // Verify the tool description
        let description = adapter.description();
        assert!(description.contains("calculate_sum"));
        assert!(description.contains("Add two numbers together"));
        
        // Verify the tool name
        let tool_name = adapter.get_tool_name();
        assert_eq!(tool_name.as_str(), "mcp_calculate_sum");
    }
    
    #[tokio::test]
    async fn test_tool_adapter_error() {
        // Create a mock MCP client that fails
        let client = Arc::new(MockToolCaller::new(true));
        
        // Create a tool definition
        let definition = ToolDefinition {
            name: "calculate_sum".to_string(),
            description: Some("Add two numbers together".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": { "type": "number" },
                    "b": { "type": "number" }
                },
                "required": ["a", "b"]
            }),
            output_schema: None,
        };
        
        // Create the adapter
        let adapter = TestMCPToolAdapter::new(client.clone(), definition);
        
        // Call the tool
        let result = adapter.call(json!({
            "a": "not a number",
            "b": 27
        })).await;
        
        // Verify the error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Tool execution failed: Error calling tool"));
    }
}