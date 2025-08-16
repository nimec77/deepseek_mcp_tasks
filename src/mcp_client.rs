use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use std::sync::Arc;

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

// JSON-RPC structures for MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub roots: Option<RootsCapability>,
    pub sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingCapability {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResponse {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub prompts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

pub struct McpClient {
    process: Arc<Mutex<Child>>,
    writer: Arc<Mutex<BufWriter<tokio::process::ChildStdin>>>,
    reader: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    next_id: Arc<Mutex<u64>>,
}

impl McpClient {
    pub async fn new(config: &Config) -> Result<Self> {
        debug!("Starting MCP server: {}", config.mcp_server_command);

        let mut child = Command::new(&config.mcp_server_command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start MCP server process")?;

        let stdin = child.stdin.take().context("Failed to get stdin from MCP server")?;
        let stdout = child.stdout.take().context("Failed to get stdout from MCP server")?;

        let writer = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let reader = Arc::new(Mutex::new(BufReader::new(stdout)));
        let process = Arc::new(Mutex::new(child));
        let next_id = Arc::new(Mutex::new(1));

        let client = Self {
            process,
            writer,
            reader,
            next_id,
        };

        // Initialize the MCP connection
        client.initialize().await?;

        info!("MCP server started and initialized successfully");
        Ok(client)
    }

    async fn initialize(&self) -> Result<()> {
        debug!("Initializing MCP connection");

        let init_request = InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities {
                roots: Some(RootsCapability { list_changed: true }),
                sampling: Some(SamplingCapability {}),
            },
            client_info: ClientInfo {
                name: "deepseek-mcp-tasks".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let response = self.send_request("initialize", Some(serde_json::to_value(init_request)?)).await?;
        
        match response.result {
            Some(_) => {
                debug!("MCP server initialized successfully");
                // Send initialized notification
                self.send_notification("notifications/initialized", None).await?;
                Ok(())
            }
            None => {
                if let Some(error) = response.error {
                    anyhow::bail!("MCP initialization failed: {}", error.message);
                } else {
                    anyhow::bail!("MCP initialization failed: no result or error");
                }
            }
        }
    }

    async fn get_next_id(&self) -> Result<String> {
        let mut id = self.next_id.lock().await;
        let current_id = *id;
        *id += 1;
        Ok(current_id.to_string())
    }

    async fn send_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<JsonRpcResponse> {
        let id = self.get_next_id().await?;
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request)?;
        debug!("Sending request: {}", request_json);

        // Send request
        {
            let mut writer = self.writer.lock().await;
            writer.write_all(request_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        // Read response
        let mut reader = self.reader.lock().await;
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        if response_line.trim().is_empty() {
            anyhow::bail!("Empty response from MCP server");
        }

        debug!("Received response: {}", response_line.trim());

        let response: JsonRpcResponse = serde_json::from_str(&response_line)
            .context("Failed to parse JSON-RPC response")?;

        if response.id != id {
            anyhow::bail!("Response ID mismatch: expected {}, got {}", id, response.id);
        }

        Ok(response)
    }

    async fn send_notification(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let request_json = serde_json::to_string(&request)?;
        debug!("Sending notification: {}", request_json);

        let mut writer = self.writer.lock().await;
        writer.write_all(request_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        Ok(())
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<Task>> {
        debug!("Fetching all tasks from MCP server");

        // For MCP, we'll use tools/call to interact with the todo system
        let params = serde_json::json!({
            "name": "get_all_tasks",
            "arguments": {}
        });

        let response = self.send_request("tools/call", Some(params)).await?;

        match response.result {
            Some(result) => {
                // Try to parse the result as tasks
                if let Some(tasks_array) = result.get("tasks") {
                    let tasks: Vec<Task> = serde_json::from_value(tasks_array.clone())
                        .context("Failed to deserialize tasks")?;
                    info!("Retrieved {} total tasks from MCP server", tasks.len());
                    Ok(tasks)
                } else {
                    // Fallback: try to parse the entire result as tasks array
                    match serde_json::from_value::<Vec<Task>>(result) {
                        Ok(tasks) => {
                            info!("Retrieved {} total tasks from MCP server", tasks.len());
                            Ok(tasks)
                        }
                        Err(_) => {
                            warn!("Could not parse tasks from MCP response, returning empty list");
                            Ok(vec![])
                        }
                    }
                }
            }
            None => {
                if let Some(error) = response.error {
                    anyhow::bail!("Failed to get tasks: {}", error.message);
                } else {
                    anyhow::bail!("No result from get_all_tasks");
                }
            }
        }
    }

    pub async fn get_tasks(&self, query: TaskQuery) -> Result<TaskListResponse> {
        debug!("Fetching tasks with query: {:?}", query);

        let params = serde_json::json!({
            "name": "get_tasks",
            "arguments": {
                "page": query.page,
                "page_size": query.page_size,
                "status": query.status,
                "priority": query.priority,
                "tag": query.tag
            }
        });

        let response = self.send_request("tools/call", Some(params)).await?;

        match response.result {
            Some(result) => {
                // Try to parse as TaskListResponse
                match serde_json::from_value::<TaskListResponse>(result.clone()) {
                    Ok(task_response) => {
                        debug!("Retrieved {} tasks from page {}", task_response.tasks.len(), task_response.page);
                        Ok(task_response)
                    }
                    Err(_) => {
                        // Fallback: create TaskListResponse from simple tasks array
                        if let Ok(tasks) = serde_json::from_value::<Vec<Task>>(result) {
                            let response = TaskListResponse {
                                tasks: tasks.clone(),
                                total: tasks.len() as u32,
                                page: query.page.unwrap_or(1),
                                page_size: query.page_size.unwrap_or(tasks.len() as u32),
                            };
                            debug!("Retrieved {} tasks from page {}", response.tasks.len(), response.page);
                            Ok(response)
                        } else {
                            anyhow::bail!("Failed to parse tasks response");
                        }
                    }
                }
            }
            None => {
                if let Some(error) = response.error {
                    anyhow::bail!("Failed to get tasks: {}", error.message);
                } else {
                    anyhow::bail!("No result from get_tasks");
                }
            }
        }
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
        debug!("Checking MCP server health");

        // Try to ping the server with a simple request
        let params = serde_json::json!({
            "name": "ping",
            "arguments": {}
        });

        match self.send_request("tools/call", Some(params)).await {
            Ok(_) => {
                info!("MCP server is healthy");
                Ok(())
            }
            Err(e) => {
                error!("MCP server health check failed: {}", e);
                anyhow::bail!("MCP server is not responding properly: {}", e)
            }
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Try to terminate the process gracefully
        if let Ok(mut process) = self.process.try_lock() {
            let _ = process.kill();
        }
    }
}
