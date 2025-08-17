use anyhow::Result;
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

use crate::tooling::{
    ChatRequest as ToolChatRequest, DeepSeekApiClient, Message, ToolObject,
    create_mcp_tool_definitions, create_task_tools, execute_mcp_tool_call, execute_task_tool,
};

const DEEPSEEK_MODEL: &str = "deepseek-chat";

/// Analysis report structure for JSON serialization
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisReport {
    /// Timestamp when the analysis was generated
    pub timestamp: DateTime<Utc>,
    /// Model used for analysis
    pub model: String,
    /// Number of tasks analyzed
    pub task_count: usize,
    /// List of tasks that were analyzed
    pub tasks: Vec<crate::mcp_client::Task>,
    /// The actual analysis content from DeepSeek
    pub analysis: String,
    /// Analysis metadata
    pub metadata: AnalysisMetadata,
}

/// Metadata about the analysis process
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    /// Whether tools were used during analysis
    pub tools_enabled: bool,
    /// Number of tool calls made during analysis
    pub tool_calls_count: Option<usize>,
    /// Duration of analysis in seconds
    pub analysis_duration_seconds: Option<f64>,
}

/// Output format for saving analysis reports
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Json,
    Markdown,
    PlainText,
}

impl OutputFormat {
    /// Determine output format from file extension
    pub fn from_path(file_path: &str) -> Self {
        let path = Path::new(file_path);
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => OutputFormat::Json,
            Some("md") | Some("markdown") => OutputFormat::Markdown,
            Some("txt") | Some("text") => OutputFormat::PlainText,
            _ => OutputFormat::Markdown, // Default to Markdown for email convenience
        }
    }
}

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

    /// Format analysis report as Markdown (email-friendly)
    pub fn format_report_as_markdown(&self, report: &AnalysisReport) -> String {
        let duration = report.metadata.analysis_duration_seconds
            .map(|d| format!("{:.1}s", d))
            .unwrap_or_else(|| "N/A".to_string());
        
        let tool_calls = report.metadata.tool_calls_count
            .map(|c| c.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        format!(
r#"# Task Analysis Report

**Generated:** {timestamp}  
**Model:** {model}  
**Tasks Analyzed:** {task_count}  
**Analysis Duration:** {duration}  
**Tool Calls:** {tool_calls}  

---

## 📋 Tasks Summary

{tasks_summary}

---

## 🤖 AI Analysis

{analysis}

---

## 📊 Report Metadata

- **Tools Enabled:** {tools_enabled}
- **Generation Time:** {timestamp}
- **Processing Duration:** {duration}
- **MCP Tool Interactions:** {tool_calls}

---

*This report was generated automatically by DeepSeek MCP Tasks analyzer.*
"#,
            timestamp = report.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            model = report.model,
            task_count = report.task_count,
            duration = duration,
            tool_calls = tool_calls,
            tasks_summary = self.format_tasks_summary(&report.tasks),
            analysis = report.analysis,
            tools_enabled = if report.metadata.tools_enabled { "Yes" } else { "No" },
        )
    }

    /// Format analysis report as plain text (maximum compatibility)
    pub fn format_report_as_text(&self, report: &AnalysisReport) -> String {
        let duration = report.metadata.analysis_duration_seconds
            .map(|d| format!("{:.1}s", d))
            .unwrap_or_else(|| "N/A".to_string());
        
        let tool_calls = report.metadata.tool_calls_count
            .map(|c| c.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        format!(
r#"===============================================
            TASK ANALYSIS REPORT
===============================================

Generated: {timestamp}
Model: {model}
Tasks Analyzed: {task_count}
Analysis Duration: {duration}
Tool Calls: {tool_calls}

===============================================
                TASKS SUMMARY
===============================================

{tasks_summary}

===============================================
               AI ANALYSIS
===============================================

{analysis}

===============================================
              REPORT METADATA
===============================================

Tools Enabled: {tools_enabled}
Generation Time: {timestamp}
Processing Duration: {duration}
MCP Tool Interactions: {tool_calls}

===============================================

This report was generated automatically by DeepSeek MCP Tasks analyzer.
"#,
            timestamp = report.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            model = report.model,
            task_count = report.task_count,
            duration = duration,
            tool_calls = tool_calls,
            tasks_summary = self.format_tasks_summary_text(&report.tasks),
            analysis = self.strip_markdown(&report.analysis),
            tools_enabled = if report.metadata.tools_enabled { "Yes" } else { "No" },
        )
    }

    /// Format tasks as a summary for Markdown
    fn format_tasks_summary(&self, tasks: &[crate::mcp_client::Task]) -> String {
        let mut summary = String::new();
        
        for (idx, task) in tasks.iter().enumerate() {
            summary.push_str(&format!("### {}. {}\n\n", idx + 1, task.title));
            
            if let Some(description) = &task.description {
                summary.push_str(&format!("**Description:** {}\n\n", description));
            }
            
            summary.push_str(&format!("**Status:** {}\n", task.status));
            
            if let Some(priority) = &task.priority {
                summary.push_str(&format!("**Priority:** {}\n", priority));
            }
            
            if let Some(due_date) = &task.due_date {
                summary.push_str(&format!("**Due Date:** {}\n", due_date));
            }
            
            if let Some(tags) = &task.tags && !tags.is_empty() {
                summary.push_str(&format!("**Tags:** {}\n", tags.join(", ")));
            }
            
            summary.push_str(&format!("**Created:** {}\n\n", task.created_at));
            summary.push_str("---\n\n");
        }
        
        summary
    }

    /// Format tasks as a summary for plain text
    fn format_tasks_summary_text(&self, tasks: &[crate::mcp_client::Task]) -> String {
        let mut summary = String::new();
        
        for (idx, task) in tasks.iter().enumerate() {
            summary.push_str(&format!("{}. {}\n", idx + 1, task.title));
            
            if let Some(description) = &task.description {
                summary.push_str(&format!("   Description: {}\n", description));
            }
            
            summary.push_str(&format!("   Status: {}\n", task.status));
            
            if let Some(priority) = &task.priority {
                summary.push_str(&format!("   Priority: {}\n", priority));
            }
            
            if let Some(due_date) = &task.due_date {
                summary.push_str(&format!("   Due Date: {}\n", due_date));
            }
            
            if let Some(tags) = &task.tags && !tags.is_empty() {
                summary.push_str(&format!("   Tags: {}\n", tags.join(", ")));
            }
            
            summary.push_str(&format!("   Created: {}\n", task.created_at));
            summary.push('\n');
        }
        
        summary
    }

    /// Strip Markdown formatting for plain text output
    fn strip_markdown(&self, markdown: &str) -> String {
        markdown
            .replace("### ", "")
            .replace("## ", "")
            .replace("# ", "")
            .replace("**", "")
            .replace("*", "")
            .replace("`", "")
            .replace("|", "")
            .replace("---", "-----------------------------------------------")
    }

    /// Save analysis report to a file in the specified format
    pub async fn save_analysis_report(
        &self,
        report: &AnalysisReport,
        file_path: &str,
    ) -> Result<()> {
        info!("Saving analysis report to {}", file_path);
        
        let format = OutputFormat::from_path(file_path);
        
        let content = match format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(report)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize analysis report: {}", e))?
            }
            OutputFormat::Markdown => self.format_report_as_markdown(report),
            OutputFormat::PlainText => self.format_report_as_text(report),
        };
        
        let path = Path::new(file_path);
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Failed to create directory {}: {}", parent.display(), e))?;
        }
        
        let mut file = File::create(path)
            .map_err(|e| anyhow::anyhow!("Failed to create file {}: {}", file_path, e))?;
        
        file.write_all(content.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to write to file {}: {}", file_path, e))?;
        
        info!("Analysis report saved successfully to {} in {:?} format", file_path, format);
        Ok(())
    }

    /// Analyze tasks using DeepSeek with MCP tools available, returning structured report
    pub async fn analyze_tasks_with_tools_report(
        &self,
        tasks: Vec<crate::mcp_client::Task>,
        mcp_client: &crate::mcp_client::McpClient,
    ) -> Result<AnalysisReport> {
        let start_time = std::time::Instant::now();
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
        let (analysis_content, tool_calls_count) = self.chat_with_tools_detailed(&analysis_prompt, &all_tools, mcp_client)
            .await?;
        
        let duration = start_time.elapsed();
        
        let report = AnalysisReport {
            timestamp: Utc::now(),
            model: self.model.clone(),
            task_count: tasks.len(),
            tasks,
            analysis: analysis_content,
            metadata: AnalysisMetadata {
                tools_enabled: true,
                tool_calls_count: Some(tool_calls_count),
                analysis_duration_seconds: Some(duration.as_secs_f64()),
            },
        };
        
        Ok(report)
    }

    /// Analyze tasks using DeepSeek with MCP tools available
    #[allow(dead_code)]
    pub async fn analyze_tasks_with_tools(
        &self,
        tasks: Vec<crate::mcp_client::Task>,
        mcp_client: &crate::mcp_client::McpClient,
    ) -> Result<String> {
        info!("Analyzing tasks with DeepSeek using MCP tools");

        // Use the detailed method for backward compatibility
        let report = self.analyze_tasks_with_tools_report(tasks, mcp_client).await?;
        Ok(report.analysis)
    }

    /// Chat with DeepSeek using available tools
    #[allow(dead_code)]
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

    /// Chat with DeepSeek using available tools, returning content and tool call count
    pub async fn chat_with_tools_detailed(
        &self,
        user_message: &str,
        tools: &[ToolObject],
        mcp_client: &crate::mcp_client::McpClient,
    ) -> Result<(String, usize)> {
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

        let mut total_tool_calls = 0;

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
                    total_tool_calls += tool_calls.len();

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
                    return Ok((content, total_tool_calls));
                }
            } else {
                anyhow::bail!("No response choices returned from DeepSeek API");
            }
        }

        warn!("Reached maximum iteration limit for tool calls");
        Ok(("Analysis completed with maximum tool call iterations reached.".to_string(), total_tool_calls))
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
