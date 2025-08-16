use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::mcp_client::McpClient;

/// DeepSeek API tool definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolObject {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// DeepSeek Chat Request structure
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolObject>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    pub temperature: f32,
    pub max_tokens: u32,
}

/// DeepSeek Chat Response structure
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// DeepSeek API client for tool-enabled interactions
pub struct DeepSeekApiClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekApiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.deepseek.com/chat/completions".to_string(),
        }
    }

    pub async fn chat_with_tools(&self, request: ChatRequest) -> Result<ChatResponse> {
        debug!("Sending chat request to DeepSeek API with {} tools", 
               request.tools.as_ref().map_or(0, |t| t.len()));

        let response = self.client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to DeepSeek API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("DeepSeek API error {}: {}", status, text);
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("Failed to parse DeepSeek API response")?;

        debug!("Received response with {} choices", chat_response.choices.len());
        Ok(chat_response)
    }
}

/// Creates a DeepSeek-compatible tool definition for invoking MCP tools
pub fn mcp_invoke_tool() -> ToolObject {
    let parameters = json!({
        "type": "object",
        "required": ["server", "tool", "arguments"],
        "properties": {
            "server": {
                "type": "string",
                "description": "MCP server alias (e.g., 'todo', 'weather', etc.)"
            },
            "tool": {
                "type": "string",
                "description": "Tool name on the MCP server to invoke"
            },
            "arguments": {
                "type": "object",
                "description": "Tool arguments as a JSON object"
            }
        }
    });

    ToolObject {
        tool_type: "function".to_string(),
        function: Function {
            name: "mcp_invoke".to_string(),
            description: "Invoke any tool on connected MCP servers to fetch data, manage tasks, or perform actions".to_string(),
            parameters,
        },
    }
}

/// Creates DeepSeek-compatible tool definitions for specific MCP tools
pub async fn create_mcp_tool_definitions(mcp_client: &McpClient) -> Result<Vec<ToolObject>> {
    info!("Creating DeepSeek tool definitions from MCP server tools");

    let mcp_tools = mcp_client.get_tools_list().await
        .context("Failed to get MCP tools list")?;

    let mut deepseek_tools = Vec::new();

    // Add the generic mcp_invoke tool
    deepseek_tools.push(mcp_invoke_tool());

    // Create specific tool definitions for each MCP tool
    for mcp_tool in mcp_tools {
        let tool_name = format!("mcp_{}", mcp_tool.name);
        let description = mcp_tool.description
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Invoke {} tool from MCP server", mcp_tool.name));

        // Convert MCP tool schema to DeepSeek tool parameters
        let mut parameters = mcp_tool.schema_as_json_value();
        
        // Ensure it has the right structure for DeepSeek API
        if !parameters.is_object() {
            parameters = json!({
                "type": "object",
                "properties": {},
                "required": []
            });
        }

        let deepseek_tool = ToolObject {
            tool_type: "function".to_string(),
            function: Function {
                name: tool_name,
                description,
                parameters,
            },
        };

        deepseek_tools.push(deepseek_tool);
        debug!("Created DeepSeek tool definition for MCP tool: {}", mcp_tool.name);
    }

    info!("Created {} DeepSeek tool definitions from MCP server", deepseek_tools.len());
    Ok(deepseek_tools)
}

/// Handles tool call execution by routing to the appropriate MCP server
pub async fn execute_mcp_tool_call(
    mcp_client: &McpClient,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value> {
    debug!("Executing MCP tool call: {} with args: {}", tool_name, arguments);

    match tool_name {
        "mcp_invoke" => {
            execute_generic_mcp_invoke(mcp_client, arguments).await
        }
        // Handle specific task tools
        "list_tasks" | "get_task" | "task_stats" => {
            execute_task_tool(mcp_client, tool_name, arguments).await
        }
        tool_name if tool_name.starts_with("mcp_") => {
            // Extract the actual MCP tool name by removing the "mcp_" prefix
            let mcp_tool_name = tool_name.strip_prefix("mcp_").unwrap();
            execute_specific_mcp_tool(mcp_client, mcp_tool_name, arguments).await
        }
        _ => {
            anyhow::bail!("Unknown tool: {}", tool_name);
        }
    }
}

/// Executes the generic mcp_invoke tool
async fn execute_generic_mcp_invoke(
    mcp_client: &McpClient,
    arguments: &Value,
) -> Result<Value> {
    let server = arguments.get("server")
        .and_then(|v| v.as_str())
        .context("Missing 'server' argument")?;

    let tool = arguments.get("tool")
        .and_then(|v| v.as_str())
        .context("Missing 'tool' argument")?;

    let tool_args = arguments.get("arguments")
        .cloned()
        .unwrap_or(json!({}));

    info!("Invoking MCP tool '{}' on server '{}' with args: {}", tool, server, tool_args);

    // For now, we assume single MCP server. In the future, this could route to different servers
    execute_specific_mcp_tool(mcp_client, tool, &tool_args).await
}

/// Executes a specific MCP tool
async fn execute_specific_mcp_tool(
    mcp_client: &McpClient,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value> {
    use rmcp::model::CallToolRequestParam;
    use std::borrow::Cow;

    debug!("Executing specific MCP tool: {} with arguments: {}", tool_name, arguments);

    // Get the peer for making requests
    let peer = {
        let client = mcp_client.client.lock().await;
        client.clone()
    };

    // Convert arguments to the format expected by rmcp
    let args = if arguments.is_object() && !arguments.as_object().unwrap().is_empty() {
        // For rmcp, we need to pass arguments as a serde_json::Map
        match arguments {
            Value::Object(map) => Some(map.clone()),
            _ => None,
        }
    } else {
        None
    };

    let params = CallToolRequestParam {
        name: Cow::Owned(tool_name.to_string()),
        arguments: args,
    };

    let result = peer.call_tool(params).await
        .context(format!("Failed to call MCP tool '{}'", tool_name))?;

    // Convert the result to a JSON value for DeepSeek
    let mut response = HashMap::new();
    
    if let Some(content_vec) = result.content
        && !content_vec.is_empty() {
            let mut content_responses = Vec::new();
            
            for content in content_vec {
                match &content.raw {
                    rmcp::model::RawContent::Text(text_content) => {
                        // Try to parse as JSON, fall back to plain text
                        match serde_json::from_str::<Value>(&text_content.text) {
                            Ok(json_value) => content_responses.push(json_value),
                            Err(_) => content_responses.push(json!({
                                "text": text_content.text,
                                "type": "text"
                            })),
                        }
                    }
                    rmcp::model::RawContent::Image(image_content) => {
                        content_responses.push(json!({
                            "data": image_content.data,
                            "mime_type": image_content.mime_type,
                            "type": "image"
                        }));
                    }
                    rmcp::model::RawContent::Resource(resource_content) => {
                        content_responses.push(json!({
                            "resource": resource_content.resource,
                            "type": "resource"
                        }));
                    }
                    rmcp::model::RawContent::Audio(audio_content) => {
                        content_responses.push(json!({
                            "data": audio_content.data,
                            "mime_type": audio_content.mime_type,
                            "type": "audio"
                        }));
                    }
                }
            }
            
            if content_responses.len() == 1 {
                response.insert("content".to_string(), content_responses.into_iter().next().unwrap());
            } else {
                response.insert("content".to_string(), json!(content_responses));
            }
        }

    // Add metadata about the tool execution
    response.insert("tool_name".to_string(), json!(tool_name));
    response.insert("success".to_string(), json!(true));

    if result.is_error.unwrap_or(false) {
        response.insert("success".to_string(), json!(false));
        response.insert("error".to_string(), json!("Tool execution reported an error"));
    }

    let response_json = json!(response);
    debug!("MCP tool '{}' execution result: {}", tool_name, response_json);

    Ok(response_json)
}

/// Creates task-specific tools for the available MCP server commands
pub fn create_task_tools() -> Vec<ToolObject> {
    vec![
        // list_tasks tool
        ToolObject {
            tool_type: "function".to_string(),
            function: Function {
                name: "list_tasks".to_string(),
                description: "List all tasks, optionally filtered by status, priority, assignee, or tag".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "assignee": {
                            "type": "string",
                            "description": "Filter tasks by assignee"
                        },
                        "priority": {
                            "type": "string",
                            "description": "Filter tasks by priority"
                        },
                        "status": {
                            "type": "string",
                            "description": "Filter tasks by status"
                        },
                        "tag": {
                            "type": "string",
                            "description": "Filter tasks by tag"
                        }
                    }
                }),
            },
        },
        
        // get_task tool
        ToolObject {
            tool_type: "function".to_string(),
            function: Function {
                name: "get_task".to_string(),
                description: "Get detailed information about a specific task by ID".to_string(),
                parameters: json!({
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "The ID of the task to retrieve"
                        }
                    }
                }),
            },
        },
        
        // task_stats tool
        ToolObject {
            tool_type: "function".to_string(),
            function: Function {
                name: "task_stats".to_string(),
                description: "Get statistics about tasks (counts by status, priority, etc.)".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        },        
    ]
}

/// Executes task-specific tool calls using the actual MCP server commands
pub async fn execute_task_tool(
    mcp_client: &McpClient,
    tool_name: &str,
    arguments: &Value,
) -> Result<Value> {
    debug!("Executing task tool: {} with arguments: {}", tool_name, arguments);

    match tool_name {
        "list_tasks" => {
            // Extract filter parameters and build arguments for the MCP tool
            let mut mcp_args = serde_json::Map::new();
            
            if let Some(assignee) = arguments.get("assignee").and_then(|v| v.as_str()) {
                mcp_args.insert("assignee".to_string(), json!(assignee));
            }
            if let Some(priority) = arguments.get("priority").and_then(|v| v.as_str()) {
                mcp_args.insert("priority".to_string(), json!(priority));
            }
            if let Some(status) = arguments.get("status").and_then(|v| v.as_str()) {
                mcp_args.insert("status".to_string(), json!(status));
            }
            if let Some(tag) = arguments.get("tag").and_then(|v| v.as_str()) {
                mcp_args.insert("tag".to_string(), json!(tag));
            }
            
            execute_specific_mcp_tool(mcp_client, "list_tasks", &json!(mcp_args)).await
        }
        
        "get_task" => {
            let id = arguments.get("id")
                .and_then(|v| v.as_str())
                .context("Missing 'id' argument for get_task")?;
            
            let mcp_args = json!({
                "id": id
            });
            
            execute_specific_mcp_tool(mcp_client, "get_task", &mcp_args).await
        }
        
        "task_stats" => {
            // task_stats takes no parameters
            execute_specific_mcp_tool(mcp_client, "task_stats", &json!({})).await
        }
                
        _ => {
            anyhow::bail!("Unknown task tool: {}", tool_name);
        }
    }
}
