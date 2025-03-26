//! Standard IO transport for MCP protocol

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio::task;

use crate::error::{MCPError, Result};
use super::Transport;

/// Transport implementation for stdio-based MCP servers
pub struct StdioTransport {
    /// Child process running the MCP server
    #[allow(dead_code)]
    child: Arc<Mutex<Child>>,
    
    /// Sender for sending messages to the server
    sender: mpsc::Sender<Message>,
    
    /// Next request ID
    next_id: AtomicU64,
}

enum Message {
    Request {
        id: u64,
        method: String,
        params: Value,
        response_tx: oneshot::Sender<Result<Value>>,
    },
    Notification {
        method: String,
        params: Value,
    },
}

impl StdioTransport {
    /// Create a new stdio transport by launching a child process
    pub async fn new(command: &str, args: &[&str]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| MCPError::TransportError(format!("Failed to start MCP server: {}", e)))?;
        
        let stdin = child.stdin.take()
            .ok_or_else(|| MCPError::TransportError("Failed to open stdin".to_string()))?;
        
        let stdout = child.stdout.take()
            .ok_or_else(|| MCPError::TransportError("Failed to open stdout".to_string()))?;
        
        let child = Arc::new(Mutex::new(child));
        
        // Create a channel for sending messages to the server
        let (sender, receiver) = mpsc::channel::<Message>(100);
        
        // Launch background worker tasks
        Self::spawn_writer(stdin, receiver);
        Self::spawn_reader(stdout, sender.clone());
        
        Ok(Self {
            child,
            sender,
            next_id: AtomicU64::new(1),
        })
    }
    
    /// Spawn a worker that writes to the server's stdin
    fn spawn_writer(stdin: ChildStdin, mut receiver: mpsc::Receiver<Message>) {
        let stdin = Arc::new(Mutex::new(stdin));
        
        task::spawn(async move {
            let mut pending_requests: HashMap<u64, oneshot::Sender<Result<Value>>> = HashMap::new();
            
            while let Some(message) = receiver.recv().await {
                match message {
                    Message::Request { id, method, params, response_tx } => {
                        let request = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": method,
                            "params": params,
                        });
                        
                        let result = {
                            let mut stdin = stdin.lock().unwrap();
                            writeln!(stdin, "{}", request).is_ok() && stdin.flush().is_ok()
                        };
                        
                        if result {
                            pending_requests.insert(id, response_tx);
                        } else {
                            let _ = response_tx.send(Err(MCPError::TransportError(
                                "Failed to write request to server".to_string()
                            )));
                        }
                    }
                    Message::Notification { method, params } => {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": method,
                            "params": params,
                        });
                        
                        let mut stdin = stdin.lock().unwrap();
                        let _ = writeln!(stdin, "{}", notification);
                        let _ = stdin.flush();
                    }
                }
            }
        });
    }
    
    /// Spawn a worker that reads from the server's stdout
    fn spawn_reader(stdout: ChildStdout, sender: mpsc::Sender<Message>) {
        let reader = BufReader::new(stdout);
        
        task::spawn(async move {
            let mut pending_requests: HashMap<u64, oneshot::Sender<Result<Value>>> = HashMap::new();
            let mut reader = reader.lines();
            
            while let Some(line_result) = task::block_in_place(|| reader.next()) {
                let line = match line_result {
                    Ok(line) => line,
                    Err(_) => continue,
                };
                let response = match serde_json::from_str::<Value>(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                
                if let Some(id) = response.get("id").and_then(|id| id.as_u64()) {
                    if let Some(tx) = pending_requests.remove(&id) {
                        if let Some(error) = response.get("error") {
                            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1) as i32;
                            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                            let _ = tx.send(Err(MCPError::ServerError(message.to_string(), code)));
                        } else if let Some(result) = response.get("result") {
                            let _ = tx.send(Ok(result.clone()));
                        } else {
                            let _ = tx.send(Err(MCPError::ProtocolError("Invalid response format".to_string())));
                        }
                    }
                }
            }
        });
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        let (response_tx, response_rx) = oneshot::channel();
        
        self.sender.send(Message::Request {
            id,
            method: method.to_string(),
            params,
            response_tx,
        }).await.map_err(|_| MCPError::TransportError("Failed to send request".to_string()))?;
        
        response_rx.await.map_err(|_| MCPError::TransportError("Failed to receive response".to_string()))?
    }
    
    async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        self.sender.send(Message::Notification {
            method: method.to_string(),
            params,
        }).await.map_err(|_| MCPError::TransportError("Failed to send notification".to_string()))?;
        
        Ok(())
    }
}