use anyhow::{Context, Result};
use rmcp::{
    model::{CallToolRequestParam, Tool},
    service::{Peer, RoleClient, ServiceExt},
    transport::TokioChildProcess,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<String>,
    pub due_date: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
    pub count: u32,
    pub filters_applied: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct TaskQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub tag: Option<String>,
}

/// Main MCP client that wraps the rmcp client and provides task-specific functionality
pub struct McpClient {
    pub client: Arc<Mutex<rmcp::service::RunningService<RoleClient, ()>>>,
}

impl McpClient {
    pub async fn new(config: &Config) -> Result<Self> {
        debug!(
            "Starting MCP server: {} {:?}",
            config.mcp_server_command, config.mcp_server_args
        );

        // Create the command for the MCP server
        let mut command = tokio::process::Command::new(&config.mcp_server_command);
        command.args(&config.mcp_server_args);

        // Create the transport using TokioChildProcess
        let transport =
            TokioChildProcess::new(command).context("Failed to create MCP server transport")?;

        // Start the client service with unit type handler
        let client = ().serve(transport).await.context("Failed to start MCP client service")?;

        info!("MCP server started and initialized successfully");

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    /// Get the peer for making requests
    async fn get_peer(&self) -> Result<Peer<RoleClient>> {
        let client = self.client.lock().await;
        // RunningService implements Deref to Peer<RoleClient>, so we can access it directly
        Ok(client.clone())
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<Task>> {
        debug!("Fetching all tasks from MCP server");

        let peer = self.get_peer().await?;

        // Call the list_tasks tool
        let params = CallToolRequestParam {
            name: Cow::Borrowed("list_tasks"),
            arguments: None,
        };

        let result = peer.call_tool(params).await?;

        // Extract content from the result
        let content = result.content;
        if let Some(content_vec) = content {
            if content_vec.is_empty() {
                anyhow::bail!("No content returned from MCP server");
            }

            // Get the first content item
            let first_content = &content_vec[0];

            // Parse the raw text content as JSON
            let json_text = match &first_content.raw {
                rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => anyhow::bail!("Expected text content from MCP server"),
            };

            // Parse the JSON text directly
            match serde_json::from_str::<TaskListResponse>(json_text) {
                Ok(task_response) => {
                    debug!(
                        "Retrieved {} tasks from MCP server",
                        task_response.tasks.len()
                    );
                    Ok(task_response.tasks)
                }
                Err(e) => {
                    error!("Failed to parse tasks response: {}", e);
                    anyhow::bail!("Failed to parse tasks response from MCP server");
                }
            }
        } else {
            anyhow::bail!("No content returned from MCP server");
        }
    }

    pub async fn get_unfinished_tasks(&self) -> Result<Vec<Task>> {
        debug!("Fetching unfinished tasks from MCP server");

        // First, let's get all tasks and filter manually since the tool interface is unclear
        debug!("Getting all tasks and filtering manually");
        let all_tasks = self.get_all_tasks().await?;
        let unfinished_tasks = all_tasks
            .into_iter()
            .filter(|task| self.is_task_unfinished(task))
            .collect::<Vec<_>>();

        info!("Found {} unfinished tasks", unfinished_tasks.len());
        Ok(unfinished_tasks)
    }

    pub async fn get_tasks_by_status(&self, status: &str) -> Result<Vec<Task>> {
        debug!("Fetching tasks with status '{}' from MCP server", status);

        // First, let's get all tasks and filter by status
        // In the future, this could be optimized to use the TaskQuery with status filter
        // if the MCP server supports it directly
        let all_tasks = self.get_all_tasks().await?;
        let filtered_tasks = all_tasks
            .into_iter()
            .filter(|task| task.status.to_lowercase() == status.to_lowercase())
            .collect::<Vec<_>>();

        info!(
            "Found {} tasks with status '{}'",
            filtered_tasks.len(),
            status
        );
        Ok(filtered_tasks)
    }

    fn is_task_unfinished(&self, task: &Task) -> bool {
        let status = task.status.to_lowercase();

        // Consider task unfinished if:
        // - Status indicates it's not complete
        // - Has no completion date but has other indicators
        match status.as_str() {
            "completed" | "done" | "finished" | "closed" | "resolved" => false,
            "pending" | "in_progress" | "todo" | "incomplete" | "new" | "open" | "active" => true,
            _ => {
                // For unknown statuses, check if there's a completion date
                task.completed_at.is_none()
            }
        }
    }

    pub async fn get_tools_list(&self) -> Result<Vec<Tool>> {
        debug!("Getting list of available tools from MCP server");

        let peer = self.get_peer().await?;

        // Use the list_tools method from rmcp with default parameters
        let result = peer.list_tools(Default::default()).await?;

        debug!("Retrieved {} tools from MCP server", result.tools.len());

        Ok(result.tools)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // The rmcp client will handle cleanup automatically
        debug!("MCP client dropped, cleaning up resources");
    }
}
