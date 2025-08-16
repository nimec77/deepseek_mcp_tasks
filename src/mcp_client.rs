use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout, Duration};
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

// Tool structures for MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListResponse {
    pub tools: Vec<Tool>,
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
    is_initialized: Arc<Mutex<bool>>,
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
        let is_initialized = Arc::new(Mutex::new(false));

        let client = Self {
            process,
            writer,
            reader,
            stderr_reader,
            next_id,
            is_initialized,
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
                roots: None,
                sampling: None,
            },
            client_info: ClientInfo {
                name: "mcp-tasks".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let response = self
            .send_request("initialize", Some(serde_json::to_value(init_request)?))
            .await?;

        match response.result {
            Some(_) => {
                debug!("MCP server initialized successfully");
                
                // Send initialized notification directly without params
                let notification = r#"{"jsonrpc":"2.0","method":"initialized"}"#;
                debug!("Sending notification: {}", notification);
                
                let mut writer = self.writer.lock().await;
                writer.write_all(notification.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                
                // Mark as initialized
                let mut initialized = self.is_initialized.lock().await;
                *initialized = true;
                
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

        // Send request with timeout
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
            match timeout(
                Duration::from_millis(500),
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
        let request_json = if let Some(params) = params {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params
            })
        } else {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": method
            })
        };

        let request_str = serde_json::to_string(&request_json)?;
        debug!("Sending notification: {}", request_str);

        let mut writer = self.writer.lock().await;
        writer.write_all(request_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        Ok(())
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<Task>> {
        debug!("Fetching all tasks from MCP server");

        let params = serde_json::json!({
            "name": "list_tasks",
            "arguments": {}
        });

        let response = self.send_request("tools/call", Some(params)).await?;

        match response.result {
            Some(result) => {
                // Try to parse as TaskListResponse first
                match serde_json::from_value::<TaskListResponse>(result.clone()) {
                    Ok(task_response) => {
                        debug!(
                            "Retrieved {} tasks from MCP server",
                            task_response.tasks.len()
                        );
                        Ok(task_response.tasks)
                    }
                    Err(_) => {
                        // Fallback: try to parse as simple tasks array
                        match serde_json::from_value::<Vec<Task>>(result) {
                            Ok(tasks) => {
                                debug!("Retrieved {} tasks from MCP server", tasks.len());
                                Ok(tasks)
                            }
                            Err(e) => {
                                error!("Failed to parse tasks response: {}", e);
                                anyhow::bail!("Failed to parse tasks response from MCP server");
                            }
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

    pub async fn get_tools_list(&self) -> Result<Vec<Tool>> {
        debug!("Getting list of available tools from MCP server");

        // Check if we're initialized
        let initialized = *self.is_initialized.lock().await;
        if !initialized {
            warn!("MCP client not initialized, attempting to initialize...");
            self.initialize().await?;
        }

        // Add a small delay to ensure the server is ready
        sleep(Duration::from_millis(100)).await;

        let response = self.send_request("tools/list", Some(serde_json::json!({}))).await?;

        match response.result {
            Some(result) => {
                // Try to parse as ToolsListResponse
                match serde_json::from_value::<ToolsListResponse>(result.clone()) {
                    Ok(tools_response) => {
                        debug!(
                            "Retrieved {} tools from MCP server",
                            tools_response.tools.len()
                        );
                        Ok(tools_response.tools)
                    }
                    Err(_) => {
                        // Fallback: try to parse as simple tools array
                        match serde_json::from_value::<Vec<Tool>>(result) {
                            Ok(tools) => {
                                debug!("Retrieved {} tools from MCP server", tools.len());
                                Ok(tools)
                            }
                            Err(e) => {
                                error!("Failed to parse tools response: {}", e);
                                anyhow::bail!("Failed to parse tools response from MCP server");
                            }
                        }
                    }
                }
            }
            None => {
                if let Some(error) = response.error {
                    anyhow::bail!("Failed to get tools list: {}", error.message);
                } else {
                    anyhow::bail!("No result from tools/list");
                }
            }
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Try to terminate the process gracefully
        // Note: Cannot use `.await` in Drop, so we must use the sync version.
        if let Ok(mut process) = self.process.try_lock() {
            // Attempt to kill the process synchronously.
            // If `kill` is async, consider providing a sync fallback or document the limitation.
            std::mem::drop(process.kill());
        }
    }
}
