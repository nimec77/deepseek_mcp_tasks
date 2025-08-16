use tabled::{Table, Tabled, settings::{Style, Alignment, Modify, object::Columns}};
use anyhow::Result;
use chrono::{DateTime, Utc};
use crate::mcp_client::Task;

#[derive(Debug, Tabled)]
pub struct TaskTableRow {
    #[tabled(rename = "ID")]
    pub id: String,
    
    #[tabled(rename = "Title")]
    pub title: String,
    
    #[tabled(rename = "Status")]
    pub status: String,
    
    #[tabled(rename = "Priority")]
    pub priority: String,
    
    #[tabled(rename = "Due Date")]
    pub due_date: String,
    
    #[tabled(rename = "Created")]
    pub created_at: String,
    
    #[tabled(rename = "Tags")]
    pub tags: String,
}

impl From<Task> for TaskTableRow {
    fn from(task: Task) -> Self {
        Self {
            id: truncate_string(&task.id, 8),
            title: truncate_string(&task.title, 40),
            status: task.status,
            priority: task.priority.unwrap_or_else(|| "N/A".to_string()),
            due_date: format_date_string(task.due_date.as_deref()),
            created_at: format_date_string(Some(&task.created_at)),
            tags: format_tags(task.tags.as_deref()),
        }
    }
}

pub struct TaskTableFormatter;

impl TaskTableFormatter {
    pub fn format_unfinished_tasks(tasks: &[Task]) -> Result<String> {
        if tasks.is_empty() {
            return Ok("No unfinished tasks found.".to_string());
        }

        let table_rows: Vec<TaskTableRow> = tasks
            .iter()
            .map(|task| TaskTableRow::from(task.clone()))
            .collect();

        let mut table = Table::new(table_rows);

        // Apply styling
        table
            .with(Style::modern())
            .with(Modify::new(Columns::single(0)).with(Alignment::center())) // ID column centered
            .with(Modify::new(Columns::single(2)).with(Alignment::center())) // Status column centered
            .with(Modify::new(Columns::single(3)).with(Alignment::center())); // Priority column centered

        let output = format!(
            "\n🎯 Unfinished Tasks ({} total)\n{}\n{}",
            tasks.len(),
            "=".repeat(80),
            table.to_string()
        );

        Ok(output)
    }

    pub fn format_summary_statistics(tasks: &[Task], total_tasks: usize) -> String {
        let unfinished_count = tasks.len();
        let completion_rate = if total_tasks > 0 {
            ((total_tasks - unfinished_count) as f64 / total_tasks as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "\n📊 Task Summary\n{}\nTotal Tasks: {}\nUnfinished Tasks: {}\nCompletion Rate: {:.1}%\n",
            "=".repeat(40),
            total_tasks,
            unfinished_count,
            completion_rate
        )
    }

    pub fn format_priority_breakdown(tasks: &[Task]) -> String {
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;
        let mut no_priority_count = 0;

        for task in tasks {
            match task.priority.as_deref().unwrap_or("").to_lowercase().as_str() {
                "high" | "urgent" | "critical" => high_count += 1,
                "medium" | "normal" => medium_count += 1,
                "low" => low_count += 1,
                _ => no_priority_count += 1,
            }
        }

        let mut output = format!("\n⚡ Priority Breakdown\n{}\n", "=".repeat(30));
        
        if high_count > 0 {
            output.push_str(&format!("🔴 High Priority: {}\n", high_count));
        }
        if medium_count > 0 {
            output.push_str(&format!("🟡 Medium Priority: {}\n", medium_count));
        }
        if low_count > 0 {
            output.push_str(&format!("🟢 Low Priority: {}\n", low_count));
        }
        if no_priority_count > 0 {
            output.push_str(&format!("⚪ No Priority Set: {}\n", no_priority_count));
        }

        output
    }

    pub fn format_overdue_tasks(tasks: &[Task]) -> Result<String> {
        let now = Utc::now();
        let overdue_tasks: Vec<&Task> = tasks
            .iter()
            .filter(|task| {
                if let Some(due_date_str) = &task.due_date {
                    if let Ok(due_date) = DateTime::parse_from_rfc3339(due_date_str) {
                        return due_date.with_timezone(&Utc) < now;
                    }
                }
                false
            })
            .collect();

        if overdue_tasks.is_empty() {
            return Ok("No overdue tasks found.".to_string());
        }

        let overdue_rows: Vec<TaskTableRow> = overdue_tasks
            .into_iter()
            .map(|task| TaskTableRow::from(task.clone()))
            .collect();

        let row_count = overdue_rows.len();
        
        let mut table = Table::new(overdue_rows);
        table
            .with(Style::modern())
            .with(Modify::new(Columns::single(0)).with(Alignment::center()))
            .with(Modify::new(Columns::single(2)).with(Alignment::center()))
            .with(Modify::new(Columns::single(3)).with(Alignment::center()));

        let table_output = table.to_string();
        
        let output = format!(
            "\n🚨 Overdue Tasks ({} total)\n{}\n{}",
            row_count,
            "=".repeat(80),
            table_output
        );

        Ok(output)
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

fn format_date_string(date_str: Option<&str>) -> String {
    match date_str {
        Some(date) => {
            // Try to parse and format the date nicely
            if let Ok(parsed_date) = DateTime::parse_from_rfc3339(date) {
                parsed_date.format("%Y-%m-%d").to_string()
            } else if let Ok(parsed_date) = DateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S") {
                parsed_date.format("%Y-%m-%d").to_string()
            } else {
                // If parsing fails, just truncate and return as-is
                truncate_string(date, 10)
            }
        }
        None => "N/A".to_string(),
    }
}

fn format_tags(tags: Option<&[String]>) -> String {
    match tags {
        Some(tag_slice) if !tag_slice.is_empty() => {
            let tags_str = tag_slice.join(", ");

            truncate_string(&tags_str, 30)
        }
        _ => "N/A".to_string(),
    }
}
