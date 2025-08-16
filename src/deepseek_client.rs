use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Context, Result};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use crate::config::Config;

#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug)]
pub struct DeepSeekClient {
    client: Client,
    config: Config,
}

impl DeepSeekClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, config })
    }

    pub async fn analyze_tasks(&self, tasks_json: &str) -> Result<String> {
        let system_prompt = r#"
You are a task analysis assistant. Your job is to analyze a list of tasks in JSON format and identify which tasks are unfinished or incomplete.

Please analyze the provided tasks and return a JSON response containing only the unfinished/incomplete tasks.

A task should be considered unfinished if:
- Its status is "pending", "in_progress", "todo", "incomplete", or similar
- Its status is not "completed", "done", "finished", or similar
- It has a completion percentage less than 100%
- It has no completion date but has a due date

Return the response in the following JSON format:
{
    "unfinished_tasks": [
        // array of unfinished task objects
    ],
    "summary": {
        "total_tasks": number,
        "unfinished_count": number,
        "completion_rate": percentage
    }
}
"#;

        let user_message = format!("Analyze these tasks:\n\n{}", tasks_json);

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_message,
            },
        ];

        self.chat_completion(messages).await
    }

    pub async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let request = ChatCompletionRequest {
            model: self.config.deepseek_model.clone(),
            messages,
            max_tokens: Some(4000),
            temperature: Some(0.1),
            stream: false,
        };

        let mut attempts = 0;
        let max_attempts = self.config.max_retries + 1;

        while attempts < max_attempts {
            attempts += 1;
            
            debug!(
                "Attempting DeepSeek API call (attempt {}/{})",
                attempts, max_attempts
            );

            match self.make_request(&request).await {
                Ok(response) => {
                    info!("DeepSeek API call successful on attempt {}", attempts);
                    return Ok(response);
                }
                Err(e) => {
                    if attempts < max_attempts {
                        warn!(
                            "DeepSeek API call failed on attempt {}/{}: {}. Retrying...",
                            attempts, max_attempts, e
                        );
                        sleep(Duration::from_millis(self.config.retry_delay * attempts as u64)).await;
                    } else {
                        error!(
                            "DeepSeek API call failed after {} attempts: {}",
                            max_attempts, e
                        );
                        return Err(e);
                    }
                }
            }
        }

        unreachable!("Should not reach here")
    }

    async fn make_request(&self, request: &ChatCompletionRequest) -> Result<String> {
        let response = self
            .client
            .post(&self.config.deepseek_api_url)
            .header("Authorization", format!("Bearer {}", self.config.deepseek_api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .context("Failed to send request to DeepSeek API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "No error message".to_string());
            anyhow::bail!(
                "DeepSeek API request failed with status {}: {}",
                status,
                error_text
            );
        }

        let chat_response: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to deserialize DeepSeek API response")?;

        if let Some(choice) = chat_response.choices.first() {
            debug!("DeepSeek API response received successfully");
            Ok(choice.message.content.clone())
        } else {
            anyhow::bail!("No choices in DeepSeek API response")
        }
    }
}
