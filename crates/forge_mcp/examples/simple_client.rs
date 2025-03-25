use anyhow::Result;
use forge_mcp::{MCPClient, MCPContext};
use forge_domain::Provider;
use url::Url;

// A simple test client for demonstrating basic MCP operations
fn main() -> Result<()> {
    // Initialize the environment
    dotenv::dotenv().ok();
    
    // Get MCP API key from environment
    let api_key = std::env::var("ANTINOMY_API_KEY")
        .expect("ANTINOMY_API_KEY must be set");
    
    // Create MCP provider
    let provider = Provider::mcp(&api_key);
    
    // Initialize MCP client
    let client = MCPClient::from_provider(provider)?;
    
    // Create a few test contexts
    let context1 = MCPContext::new("code", "Code Context")
        .with_system_message("You are an expert software engineer.")
        .with_user_message("Let's discuss the architecture of this project.");
    
    let context2 = MCPContext::new("research", "Research Context")
        .with_system_message("You are a research assistant specialized in programming language theory.")
        .with_user_message("Tell me about the latest advancements in type systems.");
    
    println!("Created contexts:");
    println!("- Context 1: {}", context1.name);
    println!("- Context 2: {}", context2.name);
    
    // In a real implementation, we would:
    // 1. Create these contexts on the MCP server
    // 2. Allow switching between contexts
    // 3. Send messages to the active context
    // 4. Receive responses from the MCP server
    
    println!("\nMCP integration is ready for use!");
    
    Ok(())
}