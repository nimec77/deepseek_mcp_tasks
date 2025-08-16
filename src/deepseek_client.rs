use anyhow::Result;
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest};
use serde_json::Value;
use std::env;
use tracing::{debug, info, warn};

use crate::tooling::{
    ChatRequest as ToolChatRequest, DeepSeekApiClient, Message, ToolObject,
    create_mcp_tool_definitions, create_task_tools, execute_mcp_tool_call, execute_task_tool,
};

const DEEPSEEK_MODEL: &str = "deepseek-chat";

pub struct DeepSeekClient {
    client: Client,
    deepseek_api: DeepSeekApiClient,
    model: String,
}

impl DeepSeekClient {
    pub fn new() -> Result<Self> {
        info!("Building DeepSeek API client...");

        // Verify API key is set
        let api_key = env::var("DEEPSEEK_API_KEY")
            .map_err(|_| anyhow::anyhow!("DEEPSEEK_API_KEY environment variable is not set"))?;

        let client = Client::default();
        let deepseek_api = DeepSeekApiClient::new(api_key);

        info!("DeepSeek client created successfully");
        Ok(Self {
            client,
            deepseek_api,
            model: DEEPSEEK_MODEL.to_string(),
        })
    }

    pub async fn analyze_tasks(&self, tasks: Vec<crate::mcp_client::Task>) -> Result<String> {
        info!("Sending tasks to DeepSeek for analysis...");

        let task_summary = self.format_tasks_for_analysis(&tasks);
        let analysis_prompt = self.create_analysis_prompt(&task_summary, tasks.len());

        let chat_req = ChatRequest::new(vec![
            ChatMessage::system(
                "You are a task analysis expert. Analyze the provided pending tasks and provide insights about priorities, dependencies, complexity, and actionable recommendations.",
            ),
            ChatMessage::user(analysis_prompt),
        ]);

        let chat_res = self.client.exec_chat(&self.model, chat_req, None).await?;

        let response_text = chat_res
            .content_text_as_str()
            .ok_or_else(|| anyhow::anyhow!("No response text received from DeepSeek"))?;

        info!("Task analysis completed successfully");
        Ok(response_text.to_string())
    }

    fn format_tasks_for_analysis(&self, tasks: &[crate::mcp_client::Task]) -> String {
        let mut formatted = String::new();

        for (idx, task) in tasks.iter().enumerate() {
            formatted.push_str(&format!("Task {}: {}\n", idx + 1, task.title));

            if let Some(description) = &task.description {
                formatted.push_str(&format!("  Description: {}\n", description));
            }

            formatted.push_str(&format!("  Status: {}\n", task.status));

            if let Some(priority) = &task.priority {
                formatted.push_str(&format!("  Priority: {}\n", priority));
            }

            if let Some(due_date) = &task.due_date {
                formatted.push_str(&format!("  Due Date: {}\n", due_date));
            }

            if let Some(tags) = &task.tags {
                formatted.push_str(&format!("  Tags: {}\n", tags.join(", ")));
            }

            formatted.push_str(&format!("  Created: {}\n", task.created_at));
            formatted.push('\n');
        }

        formatted
    }

    fn create_analysis_prompt(&self, task_summary: &str, task_count: usize) -> String {
        format!(
            "Please analyze the following {} pending tasks and provide:

1. **Priority Assessment**: Identify high-priority tasks based on due dates, dependencies, and business impact
2. **Complexity Analysis**: Categorize tasks by estimated complexity (simple, moderate, complex)
3. **Dependency Mapping**: Identify any potential task dependencies or conflicts
4. **Actionable Recommendations**: Suggest an optimal execution order and resource allocation
5. **Risk Assessment**: Highlight any tasks that might be at risk of delays or conflicts

Here are the pending tasks:

{}

Please provide a structured analysis that will help prioritize and organize the work effectively.",
            task_count,
            task_summary
        )
    }

    /// Analyze tasks using DeepSeek with MCP tools available
    pub async fn analyze_tasks_with_tools(
        &self,
        tasks: Vec<crate::mcp_client::Task>,
        mcp_client: &crate::mcp_client::McpClient,
    ) -> Result<String> {
        info!("Analyzing tasks with DeepSeek using MCP tools");

        // Get available MCP tools
        let tools = create_mcp_tool_definitions(mcp_client).await?;
        let task_tools = create_task_tools();

        let mut all_tools = tools;
        all_tools.extend(task_tools);

        let task_summary = self.format_tasks_for_analysis(&tasks);
        let analysis_prompt = format!(
            "Please analyze these {} tasks. You have access to MCP tools to get more detailed information about tasks, create task breakdowns, or perform analysis. Feel free to use any available tools to provide a comprehensive analysis.

Here are the initial tasks for reference:

{}

Provide insights about priorities, dependencies, complexity, and actionable recommendations. You can use the available tools to get more data or perform specific analysis operations.",
            tasks.len(),
            task_summary
        );

        // Start the conversation with tools available
        self.chat_with_tools(&analysis_prompt, &all_tools, mcp_client)
            .await
    }

    /// Chat with DeepSeek using available tools
    pub async fn chat_with_tools(
        &self,
        user_message: &str,
        tools: &[ToolObject],
        mcp_client: &crate::mcp_client::McpClient,
    ) -> Result<String> {
        debug!("Starting chat with {} tools available", tools.len());

        let mut messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are an AI assistant that can analyze tasks and manage todo lists. You have access to various tools to help you provide detailed, accurate information. Use tools when they can help provide better answers.".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: "user".to_string(),
                content: user_message.to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        // Try up to 5 tool call iterations to avoid infinite loops
        for iteration in 0..5 {
            debug!("Chat iteration {} starting", iteration + 1);

            let request = ToolChatRequest {
                model: self.model.clone(),
                messages: messages.clone(),
                tools: Some(tools.to_vec()),
                tool_choice: Some("auto".to_string()),
                temperature: 0.7,
                max_tokens: 4000,
            };

            let response = self.deepseek_api.chat_with_tools(request).await?;

            if let Some(choice) = response.choices.first() {
                // Check if there are tool calls to handle
                if let Some(tool_calls) = &choice.message.tool_calls {
                    // Convert response tool calls to message tool calls
                    let message_tool_calls: Vec<crate::tooling::ToolCall> = tool_calls
                        .iter()
                        .map(|tc| crate::tooling::ToolCall {
                            id: tc.id.clone(),
                            call_type: Some("function".to_string()),
                            function: crate::tooling::ToolCallFunction {
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                            },
                        })
                        .collect();

                    // Add the assistant's response with tool calls to the conversation
                    messages.push(Message {
                        role: "assistant".to_string(),
                        content: choice.message.content.clone().unwrap_or_default(),
                        tool_call_id: None,
                        tool_calls: Some(message_tool_calls),
                    });
                    info!("Processing {} tool calls", tool_calls.len());

                    // Process each tool call
                    for tool_call in tool_calls {
                        debug!("Executing tool call: {}", tool_call.function.name);

                        // Execute the tool call
                        let tool_result = self.execute_tool_call(tool_call, mcp_client).await?;

                        // Add the tool result back to the conversation
                        messages.push(Message {
                            role: "tool".to_string(),
                            content: serde_json::to_string(&tool_result)?,
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_calls: None,
                        });
                    }

                    // Continue the conversation with the tool results
                    continue;
                } else {
                    // No tool calls, add the assistant's final response and return it
                    let content = choice.message.content.clone().unwrap_or_default();
                    messages.push(Message {
                        role: "assistant".to_string(),
                        content: content.clone(),
                        tool_call_id: None,
                        tool_calls: None,
                    });
                    return Ok(content);
                }
            } else {
                anyhow::bail!("No response choices returned from DeepSeek API");
            }
        }

        warn!("Reached maximum iteration limit for tool calls");
        Ok("Analysis completed with maximum tool call iterations reached.".to_string())
    }

    /// Execute a tool call by routing it to the appropriate MCP function
    async fn execute_tool_call(
        &self,
        tool_call: &crate::tooling::ToolCall,
        mcp_client: &crate::mcp_client::McpClient,
    ) -> Result<Value> {
        let tool_name = &tool_call.function.name;
        let arguments: Value = serde_json::from_str(&tool_call.function.arguments)
            .unwrap_or_else(|_| serde_json::json!({}));

        debug!("Executing tool '{}' with args: {}", tool_name, arguments);

        match tool_name.as_str() {
            "list_tasks" | "get_task" | "task_stats" => {
                execute_task_tool(mcp_client, tool_name, &arguments).await
            }
            _ => {
                // Try to execute as an MCP tool
                execute_mcp_tool_call(mcp_client, tool_name, &arguments).await
            }
        }
    }
}
