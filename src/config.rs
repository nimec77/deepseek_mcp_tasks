use serde::{Deserialize, Serialize};
use std::env;
use anyhow::{Context, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub deepseek_api_key: String,
    pub deepseek_api_url: String,
    pub deepseek_model: String,
    pub mcp_server_command: String,
    pub request_timeout: u64,
    pub max_retries: u32,
    pub retry_delay: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            deepseek_api_key: String::new(),
            deepseek_api_url: "https://api.deepseek.com/v1/chat/completions".to_string(),
            deepseek_model: "deepseek-chat".to_string(),
            mcp_server_command: "node".to_string(),
            request_timeout: 30,
            max_retries: 3,
            retry_delay: 1000,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok(); // Load .env file if it exists

        let deepseek_api_key = env::var("DEEPSEEK_API_KEY")
            .context("DEEPSEEK_API_KEY environment variable is required")?;

        let deepseek_api_url = env::var("DEEPSEEK_API_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com/v1/chat/completions".to_string());

        let deepseek_model = env::var("DEEPSEEK_MODEL")
            .unwrap_or_else(|_| "deepseek-chat".to_string());

        let mcp_server_command = env::var("MCP_SERVER_COMMAND")
            .unwrap_or_else(|_| "node".to_string());
        

        let request_timeout = env::var("REQUEST_TIMEOUT")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .context("REQUEST_TIMEOUT must be a valid number")?;

        let max_retries = env::var("MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<u32>()
            .context("MAX_RETRIES must be a valid number")?;

        let retry_delay = env::var("RETRY_DELAY")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .context("RETRY_DELAY must be a valid number")?;

        Ok(Self {
            deepseek_api_key,
            deepseek_api_url,
            deepseek_model,
            mcp_server_command,
            request_timeout,
            max_retries,
            retry_delay,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.deepseek_api_key.is_empty() {
            anyhow::bail!("DeepSeek API key cannot be empty");
        }

        if self.deepseek_api_url.is_empty() {
            anyhow::bail!("DeepSeek API URL cannot be empty");
        }

        if self.deepseek_model.is_empty() {
            anyhow::bail!("DeepSeek model cannot be empty");
        }

        if self.mcp_server_command.is_empty() {
            anyhow::bail!("MCP server command cannot be empty");
        }

        Ok(())
    }
}
