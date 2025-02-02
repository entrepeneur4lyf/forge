use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Error;
use forge_domain::{Content, ChatCompletionMessage, Context as ChatContext, ContextMessage, Model, ModelId, Parameters, ProviderService, ResultStream, FinishReason};
use reqwest_eventsource::{Event, EventSource};
use tokio_stream::StreamExt;

#[derive(Debug, Clone)]
pub struct Ollama {
    client: Client,
    base_url: String,
}

/// Represents an Ollama chat request
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
}

/// Represents a chat message in the Ollama chat request
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Represents a chat response from Ollama
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: ChatMessage,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
}

/// Represents a model in Ollama
#[derive(Debug, Deserialize, Clone)]
pub struct OllamaModel {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
}

/// Represents an error in Ollama operations
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("HTTP Request Error: {0}")]
    HttpError(#[from] ReqwestError),

    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("API Error: {0}")]
    ApiError(String),
}

impl Ollama {
    /// Create a new Ollama client with a default base URL
    pub fn new() -> Self {
        let client = Client::builder().build().unwrap();
        Self {
            client,
            base_url: "http://localhost:11434".to_string(),
        }
    }

    /// Create a new Ollama client with a custom base URL
    pub fn with_base_url(base_url: &str) -> Self {
        let client = Client::builder().build().unwrap();
        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self.client
            .get(&url)
            .send()
            .await?
            .json::<HashMap<String, Vec<OllamaModel>>>()
            .await?;

        Ok(response.get("models").cloned().unwrap_or_default())
    }

    /// Pull a model
    pub async fn pull_model(&self, model_name: &str) -> Result<bool, OllamaError> {
        let url = format!("{}/api/pull", self.base_url);
        let payload = serde_json::json!({
            "name": model_name
        });

        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// Generate embeddings
    pub async fn generate_embedding(&self, model: &str, prompt: &str) -> Result<Vec<f32>, OllamaError> {
        let url = format!("{}/api/embeddings", self.base_url);
        let payload = serde_json::json!({
            "model": model,
            "prompt": prompt
        });

        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json::<HashMap<String, Vec<f32>>>()
            .await?;

        response.get("embedding")
            .cloned()
            .ok_or_else(|| OllamaError::ApiError("No embedding found".to_string()))
    }
}

#[async_trait::async_trait]
impl ProviderService for Ollama {
    async fn chat(&self, model_id: &ModelId, request: ChatContext) -> ResultStream<ChatCompletionMessage, Error> {
        let model_name = model_id.as_str();

        // Convert domain chat context to Ollama-specific chat messages
        let messages: Vec<ChatMessage> = request.messages.into_iter().map(|msg| {
            match msg {
                ContextMessage::ContentMessage(content_msg) => ChatMessage {
                    role: content_msg.role.to_string(),
                    content: content_msg.content.to_string(),
                },
                ContextMessage::ToolMessage(tool_msg) => ChatMessage {
                    role: "tool".to_string(),
                    content: serde_json::to_string(&tool_msg.content).unwrap(),
                }
            }
        }).collect();

        let chat_request = ChatRequest {
            model: model_name.to_string(),
            messages,
            stream: Some(true),
            format: None,
            options: None,
        };

        let url = format!("{}/api/chat", self.base_url);

        // Use an event stream approach 
        let rb = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&chat_request);
        
        let es = EventSource::new(rb)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let stream = es
            .take_while(|message| {
                !matches!(message, Err(reqwest_eventsource::Error::StreamEnded))
            })
            .filter_map(|event| match event {
                Ok(ref event) => match event {
                    Event::Open => None,
                    Event::Message(event) => {
                        // Ignore empty or "[DONE]" events
                        if ["[DONE]", ""].contains(&event.data.as_str()) {
                            return None;
                        }

                        let result = serde_json::from_str::<ChatResponse>(&event.data)
                            .map_err(|e| anyhow::anyhow!(e.to_string()))
                            .and_then(|response| {
                                Ok(ChatCompletionMessage {
                                    content: Some(Content::part(response.message.content.clone())),
                                    tool_call: vec![], // No tool calls in Ollama implementation
                                    finish_reason: response.done.then_some(FinishReason::Stop),
                                    usage: None, // Ollama doesn't provide usage information
                                })
                            });

                        Some(result)
                    }
                },
                Err(reqwest_eventsource::Error::StreamEnded) => None,
                Err(err) => Some(Err(err.into())),
            });

        Ok(Box::pin(Box::new(stream)))
    }

    async fn models(&self) -> anyhow::Result<Vec<Model>> {
        let ollama_models = self.list_models().await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let models = ollama_models.into_iter().map(|model| Model {
            id: ModelId::new(&model.name),
            name: model.name,
            description: Some(format!("Ollama model, size: {} bytes", model.size)),
        }).collect();

        Ok(models)
    }

    async fn parameters(&self, _model: &ModelId) -> anyhow::Result<Parameters> {
        // Ollama doesn't have a direct API to fetch model parameters
        // We'll return a default implementation
        Ok(Parameters::new(true))
    }
}


impl Default for Ollama {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use tokio_stream::StreamExt;
    use forge_domain::{ContentMessage, ProviderService};
    use forge_domain::Role::User;

    #[tokio::test]
    async fn test_list_models() {
        let ollama = super::Ollama::new();
        let models = ollama.list_models().await.unwrap();
        println!("Models: {:?}", models);
        assert!(!models.is_empty());
    }
    #[tokio::test]
    async fn test_chat() {
        let ollama = super::Ollama::new();
        let model_id = super::ModelId::new("llama3.2:latest");
        let context = super::ChatContext {
            messages: vec![super::ContextMessage::ContentMessage(ContentMessage {
                role: User,
                content: "Hello, Good morning llama!".to_string(),
                tool_calls: None,
            })],
            tools: vec![],
            tool_choice: None,
        };
        let mut stream = ollama.chat(&model_id, context).await.unwrap();
        while let Some(result) = stream.next().await {
            match result {
                Ok(message) => {
                    println!("Message: {:?}", message);
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
        }
    }
}
