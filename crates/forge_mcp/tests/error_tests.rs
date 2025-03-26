//! Error handling tests for the MCP client

#[cfg(test)]
mod error_tests {
    use forge_mcp::MCPError;
    
    #[test]
    fn test_error_formatting() {
        // Test protocol error formatting
        let protocol_error = MCPError::ProtocolError("Invalid message format".to_string());
        assert_eq!(protocol_error.to_string(), "Protocol error: Invalid message format");
        
        // Test transport error formatting
        let transport_error = MCPError::TransportError("Connection failed".to_string());
        assert_eq!(transport_error.to_string(), "Transport error: Connection failed");
        
        // Test parse error formatting
        let parse_error = MCPError::ParseError("Invalid JSON".to_string());
        assert_eq!(parse_error.to_string(), "Parse error: Invalid JSON");
        
        // Test unsupported operation formatting
        let unsupported_error = MCPError::UnsupportedOperation("Tools not supported".to_string());
        assert_eq!(unsupported_error.to_string(), "Unsupported operation: Tools not supported");
        
        // Test server error formatting
        let server_error = MCPError::ServerError("Method not found".to_string(), -32601);
        assert_eq!(server_error.to_string(), "Server error: Method not found (code: -32601)");
        
        // Test other error formatting
        let other_error = MCPError::Other("Unknown error".to_string());
        assert_eq!(other_error.to_string(), "Error: Unknown error");
    }
    
    #[test]
    fn test_serde_json_error_conversion() {
        // Create a serde_json::Error
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        
        // Convert to MCPError
        let mcp_error: MCPError = json_error.into();
        
        // Verify that it's a ParseError
        match mcp_error {
            MCPError::ParseError(message) => {
                assert!(message.contains("expected value at line 1 column 1"));
            },
            _ => panic!("Expected ParseError, got {:?}", mcp_error),
        }
    }
}