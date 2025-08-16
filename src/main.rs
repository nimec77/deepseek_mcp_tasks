use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::{error, info};

mod config;
mod logger;
mod deepseek_client;
mod mcp_client;
mod table_formatter;

use config::Config;
use deepseek_client::DeepSeekClient;
use mcp_client::McpClient;
use table_formatter::TaskTableFormatter;

#[derive(Parser)]
#[command(name = "deepseek-mcp-tasks")]
#[command(about = "A Rust application that integrates with MCP todo server and uses DeepSeek models")]
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
    /// List all unfinished tasks
    List {
        /// Use DeepSeek AI to analyze tasks
        #[arg(long)]
        use_ai: bool,

        /// Show detailed breakdown
        #[arg(long)]
        detailed: bool,

        /// Show overdue tasks only
        #[arg(long)]
        overdue_only: bool,
    },
    /// Check the health of MCP server
    Health,
    /// Show task statistics
    Stats,
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
            eprintln!("- DEEPSEEK_API_KEY: Your DeepSeek API key");
            eprintln!("- MCP_SERVER_URL (optional): MCP server URL (default: http://127.0.0.1:8000)");
            eprintln!("\nYou can create a .env file with these variables or export them in your shell.");
            std::process::exit(1);
        }
    };

    info!("DeepSeek MCP Tasks application started");

    match cli.command {
        Commands::List { use_ai, detailed, overdue_only } => {
            handle_list_command(config, use_ai, detailed, overdue_only).await?;
        }
        Commands::Health => {
            handle_health_command(config).await?;
        }
        Commands::Stats => {
            handle_stats_command(config).await?;
        }
    }

    Ok(())
}

async fn handle_list_command(
    config: Config,
    use_ai: bool,
    detailed: bool,
    overdue_only: bool,
) -> Result<()> {
    info!("Fetching tasks from MCP server");

    // Create MCP client
    let mcp_client = McpClient::new(&config)?;

    // Fetch unfinished tasks
    let unfinished_tasks = mcp_client.get_unfinished_tasks().await?;

    if overdue_only {
        // Show only overdue tasks
        let overdue_output = TaskTableFormatter::format_overdue_tasks(&unfinished_tasks)?;
        println!("{}", overdue_output);
        return Ok(());
    }

    if use_ai {
        info!("Using DeepSeek AI to analyze tasks");

        // Create DeepSeek client
        let deepseek_client = DeepSeekClient::new(config)?;

        // Get all tasks for AI analysis
        let all_tasks = mcp_client.get_all_tasks().await?;
        let tasks_json = serde_json::to_string_pretty(&all_tasks)?;

        // Analyze tasks with DeepSeek
        match deepseek_client.analyze_tasks(&tasks_json).await {
            Ok(ai_response) => {
                println!("\n🤖 AI Analysis Results:\n{}", "=".repeat(50));
                
                // Try to parse the AI response as JSON
                match serde_json::from_str::<serde_json::Value>(&ai_response) {
                    Ok(parsed_json) => {
                        println!("{}", serde_json::to_string_pretty(&parsed_json)?);
                    }
                    Err(_) => {
                        // If not valid JSON, just print the raw response
                        println!("{}", ai_response);
                    }
                }
            }
            Err(e) => {
                error!("AI analysis failed: {}", e);
                eprintln!("Warning: AI analysis failed, showing regular task list instead.");
            }
        }

        println!("\n");
    }

    // Show the task table
    let table_output = TaskTableFormatter::format_unfinished_tasks(&unfinished_tasks)?;
    println!("{}", table_output);

    if detailed {
        // Show additional details
        let all_tasks = mcp_client.get_all_tasks().await?;
        let summary = TaskTableFormatter::format_summary_statistics(&unfinished_tasks, all_tasks.len());
        println!("{}", summary);

        let priority_breakdown = TaskTableFormatter::format_priority_breakdown(&unfinished_tasks);
        println!("{}", priority_breakdown);
    }

    Ok(())
}

async fn handle_health_command(config: Config) -> Result<()> {
    info!("Checking MCP server health");

    let mcp_client = McpClient::new(&config)?;

    match mcp_client.health_check().await {
        Ok(()) => {
            println!("✅ MCP server is healthy and responding");
        }
        Err(e) => {
            error!("MCP server health check failed: {}", e);
            eprintln!("❌ MCP server health check failed: {}", e);
            eprintln!("Please ensure the MCP server is running at: {}", config.mcp_server_url);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_stats_command(config: Config) -> Result<()> {
    info!("Fetching task statistics");

    let mcp_client = McpClient::new(&config)?;

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
