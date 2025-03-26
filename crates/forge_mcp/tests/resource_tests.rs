//! Integration tests to ensure resources functionality works correctly

#[cfg(test)]
mod resource_tests {
    use forge_mcp::ResourceContent;

    #[test]
    fn test_resource_content_text() {
        // Create a text resource
        let content = ResourceContent {
            uri: "file:///test/document.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: Some("This is the content of the test document.".to_string()),
            blob: None,
        };
        
        // Verify the resource properties
        assert!(content.is_text());
        assert!(!content.is_binary());
        assert_eq!(
            content.as_text(), 
            Some("This is the content of the test document.")
        );
        assert_eq!(content.as_binary(), None);
    }
    
    #[test]
    fn test_resource_content_binary() {
        // Create a binary resource with base64 content
        // Base64 for "binary data" is "YmluYXJ5IGRhdGE="
        let content = ResourceContent {
            uri: "file:///test/image.png".to_string(),
            mime_type: Some("image/png".to_string()),
            text: None,
            blob: Some("YmluYXJ5IGRhdGE=".to_string()),
        };
        
        // Verify the resource properties
        assert!(!content.is_text());
        assert!(content.is_binary());
        assert_eq!(content.as_text(), None);
        
        // Let's check the binary content
        let binary = content.as_binary().expect("Failed to get binary data");
        assert_eq!(String::from_utf8_lossy(&binary), "binary data");
    }
}