use anyhow::Result;
use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;
use std::env;
use tracing::info;

const DEEPSEEK_MODEL: &str = "deepseek-chat";



pub struct DeepSeekClient {
    client: Client,
    model: String,
}

impl DeepSeekClient {
    pub fn new() -> Result<Self> {
        info!("Building DeepSeek API client...");

        // Verify API key is set
        if env::var("DEEPSEEK_API_KEY").is_err() {
            anyhow::bail!("DEEPSEEK_API_KEY environment variable is not set");
        }

        let client = Client::default();

        info!("DeepSeek client created successfully");
        Ok(Self {
            client,
            model: DEEPSEEK_MODEL.to_string(),
        })
    }

    pub async fn analyze_tasks(&self, tasks: Vec<crate::mcp_client::Task>) -> Result<String> {
        info!("Sending tasks to DeepSeek for analysis...");

        let task_summary = self.format_tasks_for_analysis(&tasks);
        let analysis_prompt = self.create_analysis_prompt(&task_summary, tasks.len());

        let chat_req = ChatRequest::new(vec![
            ChatMessage::system(
                "You are a task analysis expert. Analyze the provided pending tasks and provide insights about priorities, dependencies, complexity, and actionable recommendations."
            ),
            ChatMessage::user(analysis_prompt),
        ]);

        let chat_res = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await?;

        let response_text = chat_res
            .content_text_as_str()
            .ok_or_else(|| anyhow::anyhow!("No response text received from DeepSeek"))?;

        info!("Task analysis completed successfully");
        Ok(response_text.to_string())
    }

    fn format_tasks_for_analysis(&self, tasks: &[crate::mcp_client::Task]) -> String {
        let mut formatted = String::new();

        for (idx, task) in tasks.iter().enumerate() {
            formatted.push_str(&format!(
                "Task {}: {}\n",
                idx + 1,
                task.title
            ));

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
}


