use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

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
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
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
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub roots: Option<RootsCapability>,
    pub sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootsCapability {
    #[serde(rename = "listChanged")]
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
#[allow(dead_code)]
pub struct InitializeResponse {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ServerCapabilities {
    pub tools: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub prompts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

pub struct McpClient {
    process: Arc<Mutex<Child>>,
    writer: Arc<Mutex<BufWriter<tokio::process::ChildStdin>>>,
    reader: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    stderr_reader: Arc<Mutex<BufReader<tokio::process::ChildStderr>>>,
    next_id: Arc<Mutex<u64>>,
}

impl McpClient {
    pub async fn new(config: &Config) -> Result<Self> {
        debug!(
            "Starting MCP server: {} {:?}",
            config.mcp_server_command, config.mcp_server_args
        );

        let mut command = Command::new(&config.mcp_server_command);
        command.args(&config.mcp_server_args);

        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start MCP server process")?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to get stdin from MCP server")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to get stdout from MCP server")?;
        let stderr = child
            .stderr
            .take()
            .context("Failed to get stderr from MCP server")?;

        let writer = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let reader = Arc::new(Mutex::new(BufReader::new(stdout)));
        let stderr_reader = Arc::new(Mutex::new(BufReader::new(stderr)));
        let process = Arc::new(Mutex::new(child));
        let next_id = Arc::new(Mutex::new(1));

        let client = Self {
            process,
            writer,
            reader,
            stderr_reader,
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

        let response = self
            .send_request("initialize", Some(serde_json::to_value(init_request)?))
            .await?;

        match response.result {
            Some(_) => {
                debug!("MCP server initialized successfully");
                // Send initialized notification
                self.send_notification("initialized", None).await?;
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

    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse> {
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
        let bytes_read = reader.read_line(&mut response_line).await?;

        debug!("Read {} bytes from MCP server", bytes_read);
        if response_line.trim().is_empty() {
            // Try to read stderr to see if there's an error message
            let mut stderr_reader = self.stderr_reader.lock().await;
            let mut stderr_line = String::new();
            match tokio::time::timeout(
                std::time::Duration::from_millis(100),
                stderr_reader.read_line(&mut stderr_line),
            )
            .await
            {
                Ok(Ok(_)) if !stderr_line.trim().is_empty() => {
                    error!("MCP server stderr: {}", stderr_line.trim());
                    anyhow::bail!(
                        "Empty response from MCP server. Server error: {}",
                        stderr_line.trim()
                    );
                }
                _ => {
                    anyhow::bail!("Empty response from MCP server");
                }
            }
        }

        debug!("Received response: {}", response_line.trim());

        let response: JsonRpcResponse =
            serde_json::from_str(&response_line).context("Failed to parse JSON-RPC response")?;

        if response.id != id {
            anyhow::bail!("Response ID mismatch: expected {}, got {}", id, response.id);
        }

        Ok(response)
    }

    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method
        });

        if let Some(params) = params {
            request["params"] = params;
        }

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

        // For now, let's return some dummy tasks to test the rest of the application
        warn!("MCP server tools not yet working, returning dummy tasks for testing");
        let dummy_tasks = vec![
            Task {
                id: "1".to_string(),
                title: "Sample Todo Task".to_string(),
                description: Some("This is a sample task for testing".to_string()),
                status: "pending".to_string(),
                priority: Some("high".to_string()),
                due_date: Some("2025-01-01".to_string()),
                created_at: "2025-08-16T12:00:00Z".to_string(),
                updated_at: None,
                completed_at: None,
                tags: Some(vec!["test".to_string(), "sample".to_string()]),
            },
            Task {
                id: "2".to_string(),
                title: "Another Task".to_string(),
                description: Some("Another sample task".to_string()),
                status: "in_progress".to_string(),
                priority: Some("medium".to_string()),
                due_date: None,
                created_at: "2025-08-16T11:00:00Z".to_string(),
                updated_at: Some("2025-08-16T11:30:00Z".to_string()),
                completed_at: None,
                tags: None,
            },
        ];

        info!("Retrieved {} dummy tasks for testing", dummy_tasks.len());
        Ok(dummy_tasks)
    }

    #[allow(dead_code)]
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
                        debug!(
                            "Retrieved {} tasks from page {}",
                            task_response.tasks.len(),
                            task_response.page
                        );
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
                            debug!(
                                "Retrieved {} tasks from page {}",
                                response.tasks.len(),
                                response.page
                            );
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
