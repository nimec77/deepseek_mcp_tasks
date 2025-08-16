use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mcp_server_command: String,
    pub mcp_server_args: Vec<String>,
    pub request_timeout: u64,
    pub max_retries: u32,
    pub retry_delay: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mcp_server_command: "./mcp_todo_task".to_string(),
            mcp_server_args: vec![],
            request_timeout: 30,
            max_retries: 3,
            retry_delay: 1000,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok(); // Load .env file if it exists

        let mcp_server_command =
            env::var("MCP_SERVER_COMMAND").unwrap_or_else(|_| "./mcp_todo_task".to_string());

        let mcp_server_args = env::var("MCP_SERVER_ARGS")
            .unwrap_or_else(|_| "".to_string())
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

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
            mcp_server_command,
            mcp_server_args,
            request_timeout,
            max_retries,
            retry_delay,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.mcp_server_command.is_empty() {
            anyhow::bail!("MCP server command cannot be empty");
        }

        Ok(())
    }
}
