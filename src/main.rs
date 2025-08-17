use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info};

mod config;
mod deepseek_client;
mod logger;
mod mcp_client;
mod table_formatter;
mod tooling;

use config::Config;
use deepseek_client::DeepSeekClient;
use mcp_client::McpClient;
use table_formatter::TaskTableFormatter;

#[derive(Parser)]
#[command(name = "mcp-tasks")]
#[command(about = "A Rust application that integrates with MCP todo server")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all tasks from MCP server
    List,
    /// Get list of available tools from MCP server
    Tools,
    /// Show task statistics
    Stats,
    /// List tasks with a specific status
    Status {
        /// The status to filter by (e.g., "todo", "in_progress", "completed", "pending")
        status: String,
    },
    /// Analyze pending tasks using DeepSeek AI
    Analyze,
    /// Analyze pending tasks using DeepSeek AI with MCP tools
    AnalyzeWithTools {
        /// Optional path to save the analysis report (format auto-detected from extension: .json, .md, .txt)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    if cli.verbose {
        logger::setup_logger_with_level(tracing::Level::DEBUG)?;
    } else {
        logger::init_logger()?;
    }

    // Load configuration
    let config = match Config::from_env() {
        Ok(config) => {
            config.validate()?;
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            eprintln!("Error: {}", e);
            eprintln!("\nPlease ensure you have set the following environment variables:");
            eprintln!(
                "- MCP_SERVER_COMMAND (optional): MCP server command (default: ./mcp_todo_task)"
            );
            eprintln!("- MCP_SERVER_ARGS (optional): MCP server arguments (default: empty)");
            eprintln!(
                "\nYou can create a .env file with these variables or export them in your shell."
            );
            std::process::exit(1);
        }
    };

    info!("MCP Tasks application started");

    match cli.command {
        Commands::List => {
            handle_list_command(config).await?;
        }
        Commands::Tools => {
            handle_tools_list_command(config).await?;
        }
        Commands::Stats => {
            handle_stats_command(config).await?;
        }
        Commands::Status { status } => {
            handle_status_command(config, status).await?;
        }
        Commands::Analyze => {
            handle_analyze_command(config).await?;
        }
        Commands::AnalyzeWithTools { output } => {
            handle_analyze_with_tools_command(config, output).await?;
        }
    }

    Ok(())
}

async fn handle_analyze_command(config: Config) -> Result<()> {
    info!("Starting DeepSeek analysis of pending tasks");

    // Create MCP client
    let mcp_client = McpClient::new(&config).await?;

    // Fetch pending tasks
    let pending_tasks = mcp_client.get_tasks_by_status("pending").await?;

    if pending_tasks.is_empty() {
        println!("🎉 No pending tasks found to analyze!");
        return Ok(());
    }

    info!("Found {} pending tasks for analysis", pending_tasks.len());

    // Create DeepSeek client
    let deepseek_client = DeepSeekClient::new().map_err(|e| {
        error!("Failed to create DeepSeek client: {}", e);
        eprintln!("❌ Failed to initialize DeepSeek client: {}", e);
        eprintln!("\nPlease ensure you have set the DEEPSEEK_API_KEY environment variable.");
        eprintln!("You can add it to your .env file or export it in your shell:");
        eprintln!("export DEEPSEEK_API_KEY=your_api_key_here");
        e
    })?;

    // Show pending tasks before analysis
    println!("\n📋 Found {} pending tasks:", pending_tasks.len());
    for (idx, task) in pending_tasks.iter().enumerate() {
        println!("  {}. {} (Status: {})", idx + 1, task.title, task.status);
        if let Some(priority) = &task.priority {
            println!("     Priority: {}", priority);
        }
        if let Some(due_date) = &task.due_date {
            println!("     Due: {}", due_date);
        }
    }

    println!("\n🤖 Analyzing tasks with DeepSeek AI...\n");

    // Analyze the tasks using DeepSeek
    match deepseek_client.analyze_tasks(pending_tasks).await {
        Ok(analysis) => {
            println!("📊 DeepSeek Analysis Results:\n");
            println!("{}", analysis);
        }
        Err(e) => {
            error!("DeepSeek analysis failed: {}", e);
            eprintln!("❌ Failed to analyze tasks: {}", e);
            eprintln!("\nPlease check:");
            eprintln!("1. Your DEEPSEEK_API_KEY is valid");
            eprintln!("2. You have sufficient API credits");
            eprintln!("3. Your internet connection is working");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_analyze_with_tools_command(config: Config, output_file: Option<String>) -> Result<()> {
    info!("Starting DeepSeek analysis with MCP tools");

    // Create MCP client
    let mcp_client = McpClient::new(&config).await?;

    // Fetch pending tasks
    let pending_tasks = mcp_client.get_tasks_by_status("pending").await?;

    if pending_tasks.is_empty() {
        println!("🎉 No pending tasks found to analyze!");
        return Ok(());
    }

    info!(
        "Found {} pending tasks for tool-enabled analysis",
        pending_tasks.len()
    );

    // Create DeepSeek client
    let deepseek_client = DeepSeekClient::new().map_err(|e| {
        error!("Failed to create DeepSeek client: {}", e);
        eprintln!("❌ Failed to initialize DeepSeek client: {}", e);
        eprintln!("\nPlease ensure you have set the DEEPSEEK_API_KEY environment variable.");
        eprintln!("You can add it to your .env file or export it in your shell:");
        eprintln!("export DEEPSEEK_API_KEY=your_api_key_here");
        e
    })?;

    // Show pending tasks before analysis
    println!("\n📋 Found {} pending tasks:", pending_tasks.len());
    for (idx, task) in pending_tasks.iter().enumerate() {
        println!("  {}. {} (Status: {})", idx + 1, task.title, task.status);
        if let Some(priority) = &task.priority {
            println!("     Priority: {}", priority);
        }
        if let Some(due_date) = &task.due_date {
            println!("     Due: {}", due_date);
        }
    }

    println!("\n🚀 Analyzing tasks with DeepSeek AI using MCP tools...");
    println!("📡 The AI can now query the MCP server directly for real-time task data!\n");

    // Analyze the tasks using DeepSeek with MCP tools
    match deepseek_client
        .analyze_tasks_with_tools_report(pending_tasks, &mcp_client)
        .await
    {
        Ok(report) => {
            println!("🔧 DeepSeek Analysis with MCP Tools:\n");
            println!("{}", report.analysis);
            
            // Save to file if output path is specified
            if let Some(output_path) = output_file {
                match deepseek_client.save_analysis_report(&report, &output_path).await {
                    Ok(_) => {
                        let format_desc = match output_path.rsplit('.').next() {
                            Some("json") => "JSON format (structured data)",
                            Some("md") | Some("markdown") => "Markdown format (email-friendly)",
                            Some("txt") | Some("text") => "Plain text format (universal compatibility)",
                            _ => "Markdown format (email-friendly, default)",
                        };
                        
                        println!("\n💾 Analysis report saved to: {}", output_path);
                        println!("📧 Format: {}", format_desc);
                        info!("Report saved with {} tasks and {} tool calls", 
                              report.task_count, 
                              report.metadata.tool_calls_count.unwrap_or(0));
                    }
                    Err(e) => {
                        error!("Failed to save analysis report: {}", e);
                        eprintln!("⚠️  Warning: Failed to save report to {}: {}", output_path, e);
                        eprintln!("Analysis completed successfully but report could not be saved.");
                    }
                }
            }
        }
        Err(e) => {
            error!("DeepSeek tool-enabled analysis failed: {}", e);
            eprintln!("❌ Failed to analyze tasks with tools: {}", e);
            eprintln!("\nPlease check:");
            eprintln!("1. Your DEEPSEEK_API_KEY is valid");
            eprintln!("2. You have sufficient API credits");
            eprintln!("3. Your internet connection is working");
            eprintln!("4. The MCP server is running correctly");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_list_command(config: Config) -> Result<()> {
    info!("Fetching tasks from MCP server");

    // Create MCP client
    let mcp_client = McpClient::new(&config).await?;

    // Fetch all tasks
    let all_tasks = mcp_client.get_all_tasks().await?;

    // Show the task table
    let table_output = TaskTableFormatter::format_all_tasks(&all_tasks)?;
    println!("{}", table_output);

    Ok(())
}

async fn handle_tools_list_command(config: Config) -> Result<()> {
    info!("Getting list of available tools from MCP server");

    let mcp_client = McpClient::new(&config).await?;

    match mcp_client.get_tools_list().await {
        Ok(tools) => {
            if tools.is_empty() {
                println!("No tools available on the MCP server");
            } else {
                println!("Available tools on MCP server:");
                println!();
                for (index, tool) in tools.iter().enumerate() {
                    println!("{}. {}", index + 1, tool.name);
                    if let Some(description) = &tool.description {
                        println!("   Description: {}", description);
                    } else {
                        println!("   Description: <No description available>");
                    }
                    let schema_value = tool.schema_as_json_value();
                    if let Some(properties) = schema_value.get("properties")
                        && let Some(props_obj) = properties.as_object()
                        && !props_obj.is_empty()
                    {
                        println!(
                            "   Parameters: {}",
                            props_obj.keys().cloned().collect::<Vec<_>>().join(", ")
                        );
                    }
                    println!();
                }
            }
        }
        Err(e) => {
            error!("Failed to get tools list: {}", e);
            eprintln!("❌ Failed to get tools list: {}", e);
            eprintln!(
                "Please ensure the MCP server command is correct: {}",
                config.mcp_server_command
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_stats_command(config: Config) -> Result<()> {
    info!("Fetching task statistics");

    let mcp_client = McpClient::new(&config).await?;

    // Fetch all tasks
    let all_tasks = mcp_client.get_all_tasks().await?;
    let unfinished_tasks = mcp_client.get_unfinished_tasks().await?;

    // Display statistics
    let summary = TaskTableFormatter::format_summary_statistics(&unfinished_tasks, all_tasks.len());
    println!("{}", summary);

    let priority_breakdown = TaskTableFormatter::format_priority_breakdown(&unfinished_tasks);
    println!("{}", priority_breakdown);

    // Show overdue tasks count
    let overdue_output = TaskTableFormatter::format_overdue_tasks(&unfinished_tasks)?;
    if !overdue_output.contains("No overdue tasks found") {
        println!("{}", overdue_output);
    } else {
        println!("\n✅ No overdue tasks found!");
    }

    Ok(())
}

async fn handle_status_command(config: Config, status: String) -> Result<()> {
    info!("Fetching tasks with status '{}' from MCP server", status);

    // Create MCP client
    let mcp_client = McpClient::new(&config).await?;

    // Fetch tasks by status
    let filtered_tasks = mcp_client.get_tasks_by_status(&status).await?;

    if filtered_tasks.is_empty() {
        println!("No tasks found with status '{}'", status);
        return Ok(());
    }

    // Show the filtered task table
    let table_output = TaskTableFormatter::format_tasks_by_status(&filtered_tasks, &status)?;
    println!("{}", table_output);

    Ok(())
}
