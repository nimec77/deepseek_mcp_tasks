use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info};

mod config;
mod logger;
mod mcp_client;
mod table_formatter;

use config::Config;
use mcp_client::McpClient;
use table_formatter::TaskTableFormatter;

#[derive(Parser)]
#[command(name = "mcp-tasks")]
#[command(
    about = "A Rust application that integrates with MCP todo server"
)]
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
        /// Show detailed breakdown
        #[arg(long)]
        detailed: bool,

        /// Show overdue tasks only
        #[arg(long)]
        overdue_only: bool,
    },
    /// Get list of available tools from MCP server
    Tools,
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
            eprintln!("- MCP_SERVER_COMMAND (optional): MCP server command (default: ./mcp_todo_task)");
            eprintln!(
                "- MCP_SERVER_ARGS (optional): MCP server arguments (default: empty)"
            );
            eprintln!(
                "\nYou can create a .env file with these variables or export them in your shell."
            );
            std::process::exit(1);
        }
    };

    info!("MCP Tasks application started");

    match cli.command {
        Commands::List {
            detailed,
            overdue_only,
        } => {
            handle_list_command(config, detailed, overdue_only).await?;
        }
        Commands::Tools => {
            handle_tools_list_command(config).await?;
        }
        Commands::Stats => {
            handle_stats_command(config).await?;
        }
    }

    Ok(())
}

async fn handle_list_command(config: Config, detailed: bool, overdue_only: bool) -> Result<()> {
    info!("Fetching tasks from MCP server");

    // Create MCP client
    let mcp_client = McpClient::new(&config).await?;

    // Fetch unfinished tasks
    let unfinished_tasks = mcp_client.get_unfinished_tasks().await?;

    if overdue_only {
        // Show only overdue tasks
        let overdue_output = TaskTableFormatter::format_overdue_tasks(&unfinished_tasks)?;
        println!("{}", overdue_output);
        return Ok(());
    }

    // Show the task table
    let table_output = TaskTableFormatter::format_unfinished_tasks(&unfinished_tasks)?;
    println!("{}", table_output);

    if detailed {
        // Show additional details
        let all_tasks = mcp_client.get_all_tasks().await?;
        let summary =
            TaskTableFormatter::format_summary_statistics(&unfinished_tasks, all_tasks.len());
        println!("{}", summary);

        let priority_breakdown = TaskTableFormatter::format_priority_breakdown(&unfinished_tasks);
        println!("{}", priority_breakdown);
    }

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
                            && !props_obj.is_empty() {
                                println!("   Parameters: {}", props_obj.keys().cloned().collect::<Vec<_>>().join(", "));
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
