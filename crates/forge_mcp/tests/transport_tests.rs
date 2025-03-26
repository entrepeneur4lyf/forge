//! Integration tests for MCP transport functionality

#[cfg(test)]
mod transport_tests {
    use std::sync::{Arc, Mutex};
    use std::collections::HashMap;
    use std::io::Read;
    use std::process::Command;
    use std::thread;
    
    use serde_json::json;
    use tokio::sync::oneshot;
    
    use forge_mcp::transport::{Transport, StdioTransport};

    // A simple echo server implementation for testing StdioTransport
    struct TestServer {
        pub process: Option<std::process::Child>,
    }
    
    impl TestServer {
        fn new() -> Self {
            Self { process: None }
        }
        
        // Start a simple Rust program that echoes back JSON-RPC requests
        fn start(&mut self) -> String {
            // Create a simple echo server script
            let script = r#"
                use std::io::{self, BufRead, Write};
                use serde_json::{Value, json};
                
                fn main() {
                    let stdin = io::stdin();
                    let mut stdout = io::stdout();
                    
                    for line in stdin.lock().lines() {
                        let line = line.expect("Failed to read line");
                        let request: Value = serde_json::from_str(&line).expect("Failed to parse JSON");
                        
                        // If it's a request, send back a response
                        if let Some(id) = request.get("id") {
                            let method = request["method"].as_str().unwrap_or("");
                            
                            // For initialize, return capabilities
                            if method == "initialize" {
                                let response = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "capabilities": {
                                            "resources": {},
                                            "tools": {}
                                        }
                                    }
                                });
                                
                                writeln!(stdout, "{}", response).expect("Failed to write response");
                                stdout.flush().expect("Failed to flush stdout");
                            } else {
                                // For other methods, echo back the parameters
                                let response = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": request.get("params").unwrap_or(&json!({}))
                                });
                                
                                writeln!(stdout, "{}", response).expect("Failed to write response");
                                stdout.flush().expect("Failed to flush stdout");
                            }
                        }
                        // Ignore notifications
                    }
                }
            "#;
            
            // Write the script to a temporary file
            let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
            let script_path = temp_dir.path().join("echo_server.rs");
            
            std::fs::write(&script_path, script).expect("Failed to write script file");
            
            // Compile the script
            let binary_path = temp_dir.path().join("echo_server");
            
            Command::new("rustc")
                .args(&[script_path.to_str().unwrap(), "-o", binary_path.to_str().unwrap()])
                .output()
                .expect("Failed to compile echo server");
            
            // Return the path to the binary
            binary_path.to_str().unwrap().to_string()
        }
        
        fn stop(&mut self) {
            if let Some(mut process) = self.process.take() {
                let _ = process.kill();
                let _ = process.wait();
            }
        }
    }
    
    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stop();
        }
    }
    
    // These tests require a real Rust compiler to be available, so they're marked as ignored
    #[tokio::test]
    #[ignore]
    async fn test_stdio_transport_initialization() {
        // Start the echo server
        let mut server = TestServer::new();
        let binary_path = server.start();
        
        // Connect to the server
        let transport = StdioTransport::new(&binary_path, &[])
            .await
            .expect("Failed to create StdioTransport");
        
        // Send an initialize request
        let response = transport.send_request(
            "initialize",
            json!({
                "protocol": {
                    "version": "0.2.0"
                },
                "client": {
                    "name": "test-client",
                    "version": "0.1.0"
                },
                "capabilities": {}
            })
        ).await.expect("Failed to send initialize request");
        
        // Verify the response
        let capabilities = response.get("capabilities").expect("No capabilities in response");
        assert!(capabilities.get("resources").is_some());
        assert!(capabilities.get("tools").is_some());
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_stdio_transport_request_response() {
        // Start the echo server
        let mut server = TestServer::new();
        let binary_path = server.start();
        
        // Connect to the server
        let transport = StdioTransport::new(&binary_path, &[])
            .await
            .expect("Failed to create StdioTransport");
        
        // Send a request
        let response = transport.send_request(
            "test_method",
            json!({
                "param1": "value1",
                "param2": 42
            })
        ).await.expect("Failed to send request");
        
        // Verify the response (the echo server returns the params)
        assert_eq!(response.get("param1").and_then(|v| v.as_str()), Some("value1"));
        assert_eq!(response.get("param2").and_then(|v| v.as_i64()), Some(42));
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_stdio_transport_notification() {
        // Start the echo server
        let mut server = TestServer::new();
        let binary_path = server.start();
        
        // Connect to the server
        let transport = StdioTransport::new(&binary_path, &[])
            .await
            .expect("Failed to create StdioTransport");
        
        // Send a notification (no response expected)
        transport.send_notification(
            "test_notification",
            json!({
                "param1": "value1",
                "param2": 42
            })
        ).await.expect("Failed to send notification");
        
        // No way to verify this with the current echo server, but it should not error
    }
}