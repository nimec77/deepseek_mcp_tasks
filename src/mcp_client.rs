use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Context, Result};
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
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Serialize)]
pub struct TaskQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug)]
pub struct McpClient {
    client: Client,
    base_url: String,
}

impl McpClient {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::new();
        let base_url = config.mcp_server_url.clone();

        Ok(Self { client, base_url })
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<Task>> {
        let mut all_tasks = Vec::new();
        let mut page = 1;
        let page_size = 100;

        loop {
            debug!("Fetching tasks page {}", page);

            let query = TaskQuery {
                page: Some(page),
                page_size: Some(page_size),
                status: None,
                priority: None,
                tag: None,
            };

            let response = self.get_tasks(query).await?;
            
            if response.tasks.is_empty() {
                break;
            }

            let tasks_len = response.tasks.len();
            all_tasks.extend(response.tasks);

            // If we got fewer tasks than the page size, we've reached the end
            if tasks_len < page_size as usize {
                break;
            }

            page += 1;
        }

        info!("Retrieved {} total tasks from MCP server", all_tasks.len());
        Ok(all_tasks)
    }

    pub async fn get_tasks(&self, query: TaskQuery) -> Result<TaskListResponse> {
        let mut url = format!("{}/tasks", self.base_url);
        let mut params = Vec::new();

        if let Some(page) = query.page {
            params.push(format!("page={}", page));
        }

        if let Some(page_size) = query.page_size {
            params.push(format!("page_size={}", page_size));
        }

        if let Some(status) = query.status {
            params.push(format!("status={}", status));
        }

        if let Some(priority) = query.priority {
            params.push(format!("priority={}", priority));
        }

        if let Some(tag) = query.tag {
            params.push(format!("tag={}", tag));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        debug!("Making request to: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .send()
            .await
            .context("Failed to send request to MCP server")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "No error message".to_string());
            anyhow::bail!(
                "MCP server request failed with status {}: {}",
                status,
                error_text
            );
        }

        let task_response: TaskListResponse = response
            .json()
            .await
            .context("Failed to deserialize MCP server response")?;

        debug!("Retrieved {} tasks from page {}", task_response.tasks.len(), task_response.page);

        Ok(task_response)
    }

    pub async fn get_unfinished_tasks(&self) -> Result<Vec<Task>> {
        debug!("Fetching unfinished tasks from MCP server");

        // Try to get tasks with incomplete status first
        let incomplete_statuses = vec!["pending", "in_progress", "todo", "incomplete", "new"];
        let mut unfinished_tasks = Vec::new();

        for status in incomplete_statuses {
            let query = TaskQuery {
                page: Some(1),
                page_size: Some(1000),
                status: Some(status.to_string()),
                priority: None,
                tag: None,
            };

            match self.get_tasks(query).await {
                Ok(response) => {
                    unfinished_tasks.extend(response.tasks);
                }
                Err(e) => {
                    // If filtering by status doesn't work, we'll fall back to getting all tasks
                    debug!("Failed to filter by status '{}': {}", status, e);
                    break;
                }
            }
        }

        // If we didn't get any tasks by filtering, get all tasks and filter manually
        if unfinished_tasks.is_empty() {
            debug!("Falling back to manual filtering of all tasks");
            let all_tasks = self.get_all_tasks().await?;
            unfinished_tasks = all_tasks
                .into_iter()
                .filter(|task| self.is_task_unfinished(task))
                .collect();
        }

        info!("Found {} unfinished tasks", unfinished_tasks.len());
        Ok(unfinished_tasks)
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

    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/health", self.base_url);
        
        debug!("Checking MCP server health at: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to MCP server")?;

        if response.status().is_success() {
            info!("MCP server is healthy");
            Ok(())
        } else {
            error!("MCP server health check failed with status: {}", response.status());
            anyhow::bail!("MCP server is not responding properly")
        }
    }
}
