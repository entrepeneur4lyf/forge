//! Unit tests for MCP client functionality

#[cfg(test)]
mod mcp_client_tests {
    use std::sync::{Arc, Mutex};
    use std::collections::HashMap;
    
    use serde_json::{json, Value};
    use tokio::sync::oneshot;
    use tokio::time::{sleep, Duration};
    
    use forge_mcp::{MCPClient};
    use forge_mcp::transport::Transport;

    // A mock transport implementation for testing
    #[derive(Clone)]struct MockTransport {
        // Store requests that have been made for verification
        requests: Arc<Mutex<Vec<(String, Value)>>>,
        
        // Set up predefined responses for testing
        responses: Arc<Mutex<HashMap<String, Value>>>,
    }
    
    impl MockTransport {
        fn new() -> Self {
            Self {
                requests: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(HashMap::new())),
            }
        }
        
        // Set up a predefined response for a method
        fn respond_to(&self, method: &str, response: Value) {
            self.responses.lock().unwrap().insert(method.to_string(), response);
        }
        
        // Get the requests that have been made
        fn get_requests(&self) -> Vec<(String, Value)> {
            self.requests.lock().unwrap().clone()
        }
    }
    
    #[async_trait::async_trait]
    impl Transport for MockTransport {
        async fn send_request(&self, method: &str, params: Value) -> forge_mcp::Result<Value> {
            // Record the request
            {
                let mut requests = self.requests.lock().unwrap();
                requests.push((method.to_string(), params.clone()));
            }
            
            // Get the predefined response or return an error
            let response = {
                let responses = self.responses.lock().unwrap();
                responses.get(method).cloned().unwrap_or_else(|| {
                    json!({
                        "error": {
                            "code": -32601,
                            "message": "Method not found"
                        }
                    })
                })
            };
            
            // Simulate network delay
            sleep(Duration::from_millis(10)).await;
            
            // If the response contains an error, return the error
            if let Some(error) = response.get("error") {
                let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1) as i32;
                let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                return Err(forge_mcp::MCPError::ServerError(message.to_string(), code));
            }
            
            Ok(response)
        }
        
        async fn send_notification(&self, method: &str, params: Value) -> forge_mcp::Result<()> {
            // Record the notification
            {
                let mut requests = self.requests.lock().unwrap();
                requests.push((method.to_string(), params));
            }
            
            // Simulate network delay
            sleep(Duration::from_millis(10)).await;
            
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_client_initialization() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {
                "resources": {},
                "tools": {}
            }
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport).await.expect("Failed to connect");
        
        // Verify the client has the correct capabilities
        assert!(client.capabilities().resources.is_some());
        assert!(client.capabilities().tools.is_some());
        assert!(client.capabilities().prompts.is_none());
        assert!(client.capabilities().sampling.is_none());
    }
    
    #[tokio::test]
    async fn test_list_resources() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {
                "resources": {}
            }
        }));
        
        // Set up the expected list_resources response
        transport.respond_to("resources/list", json!({
            "resources": [
                {
                    "uri": "file:///test/document.txt",
                    "name": "Test Document",
                    "description": "A test document",
                    "mimeType": "text/plain"
                },
                {
                    "uri": "file:///test/image.png",
                    "name": "Test Image",
                    "mimeType": "image/png"
                }
            ]
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport).await.expect("Failed to connect");
        
        // List resources
        let resources = client.list_resources().await.expect("Failed to list resources");
        
        // Verify the resources
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].uri, "file:///test/document.txt");
        assert_eq!(resources[0].name, "Test Document");
        assert_eq!(resources[0].description, Some("A test document".to_string()));
        assert_eq!(resources[0].mime_type, Some("text/plain".to_string()));
        
        assert_eq!(resources[1].uri, "file:///test/image.png");
        assert_eq!(resources[1].name, "Test Image");
        assert!(resources[1].description.is_none());
        assert_eq!(resources[1].mime_type, Some("image/png".to_string()));
    }
    
    #[tokio::test]
    async fn test_read_resource() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {
                "resources": {}
            }
        }));
        
        // Set up the expected read_resource response
        transport.respond_to("resources/read", json!({
            "contents": [
                {
                    "uri": "file:///test/document.txt",
                    "mimeType": "text/plain",
                    "text": "This is the content of the test document."
                }
            ]
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport.clone()).await.expect("Failed to connect");
        
        // Read resource
        let contents = client.read_resource("file:///test/document.txt").await.expect("Failed to read resource");
        
        // Verify the resource content
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].uri, "file:///test/document.txt");
        assert_eq!(contents[0].mime_type, Some("text/plain".to_string()));
        assert_eq!(contents[0].text, Some("This is the content of the test document.".to_string()));
        assert!(contents[0].blob.is_none());
        
        // Verify that the correct request was sent
        let requests = transport.get_requests();
        let read_request = requests.iter().find(|(method, _)| method == "resources/read").expect("No resources/read request found");
        
        let params = &read_request.1;
        assert_eq!(params.get("uri").and_then(|uri| uri.as_str()), Some("file:///test/document.txt"));
    }
    
    #[tokio::test]
    async fn test_list_tools() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {
                "tools": {}
            }
        }));
        
        // Set up the expected list_tools response
        transport.respond_to("tools/list", json!({
            "tools": [
                {
                    "name": "calculate_sum",
                    "description": "Add two numbers together",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "a": { "type": "number" },
                            "b": { "type": "number" }
                        },
                        "required": ["a", "b"]
                    }
                },
                {
                    "name": "fetch_data",
                    "description": "Fetch data from a URL",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" }
                        },
                        "required": ["url"]
                    }
                }
            ]
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport).await.expect("Failed to connect");
        
        // List tools
        let tools = client.list_tools().await.expect("Failed to list tools");
        
        // Verify the tools
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "calculate_sum");
        assert_eq!(tools[0].description, Some("Add two numbers together".to_string()));
        
        assert_eq!(tools[1].name, "fetch_data");
        assert_eq!(tools[1].description, Some("Fetch data from a URL".to_string()));
    }
    
    #[tokio::test]
    async fn test_call_tool() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {
                "tools": {}
            }
        }));
        
        // Set up the expected call_tool response
        transport.respond_to("tools/call", json!({
            "content": [
                {
                    "type": "text",
                    "text": "42"
                }
            ]
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport.clone()).await.expect("Failed to connect");
        
        // Call tool
        let result = client.call_tool("calculate_sum", json!({
            "a": 15,
            "b": 27
        })).await.expect("Failed to call tool");
        
        // Verify the tool result
        assert!(!result.is_error);
        assert_eq!(result.content, "42");
        
        // Verify that the correct request was sent
        let requests = transport.get_requests();
        let call_request = requests.iter().find(|(method, _)| method == "tools/call").expect("No tools/call request found");
        
        let params = &call_request.1;
        assert_eq!(params.get("name").and_then(|name| name.as_str()), Some("calculate_sum"));
        
        let args = params.get("arguments").expect("No arguments in request");
        assert_eq!(args.get("a").and_then(|a| a.as_i64()), Some(15));
        assert_eq!(args.get("b").and_then(|b| b.as_i64()), Some(27));
    }
    
    #[tokio::test]
    async fn test_tool_error() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {
                "tools": {}
            }
        }));
        
        // Set up the expected call_tool error response
        transport.respond_to("tools/call", json!({
            "isError": true,
            "content": [
                {
                    "type": "text",
                    "text": "Invalid arguments"
                }
            ]
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport).await.expect("Failed to connect");
        
        // Call tool
        let result = client.call_tool("calculate_sum", json!({
            "a": "not a number",
            "b": 27
        })).await.expect("Failed to call tool");
        
        // Verify the tool result
        assert!(result.is_error);
        assert_eq!(result.content, "Invalid arguments");
    }
    
    #[tokio::test]
    async fn test_unsupported_capability() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response with no resources capability
        transport.respond_to("initialize", json!({
            "capabilities": {
                "tools": {}
                // No resources capability
            }
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport).await.expect("Failed to connect");
        
        // Attempt to list resources (should fail)
        let result = client.list_resources().await;
        
        // Verify the error
        assert!(result.is_err());
        match result {
            Err(forge_mcp::MCPError::UnsupportedOperation(_)) => {
                // Expected error
            },
            _ => panic!("Expected UnsupportedOperation error")
        }
    }
    
    #[tokio::test]
    async fn test_close_connection() {
        // Set up the mock transport
        let transport = Box::new(MockTransport::new());
        
        // Set up the expected initialize response
        transport.respond_to("initialize", json!({
            "capabilities": {}
        }));
        
        // Initialize the client
        let client = MCPClient::connect(transport.clone()).await.expect("Failed to connect");
        
        // Close the connection
        client.close().await.expect("Failed to close connection");
        
        // Verify that the exit notification was sent
        let requests = transport.get_requests();
        assert!(requests.iter().any(|(method, _)| method == "exit"), "No exit notification found");
    }
}