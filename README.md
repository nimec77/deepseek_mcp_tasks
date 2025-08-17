# DeepSeek MCP Tasks

A Rust application that integrates with an MCP (Model Context Protocol) todo server to display, manage, and analyze tasks using DeepSeek AI. The application provides both traditional task management capabilities and AI-powered task analysis.

## Features

- 🚀 **MCP Integration**: Connects to local MCP todo task server
- 🤖 **DeepSeek AI Integration**: AI-powered task analysis and recommendations
- 🔧 **Tool-Enabled AI**: DeepSeek can interact with MCP tools for real-time data
- 📊 **Rich Tables**: Beautiful table formatting with task details
- ⚡ **Fast & Efficient**: Built with async Rust for performance
- 📈 **Statistics**: Comprehensive task statistics and breakdowns
- 🎯 **Filtering**: Support for overdue tasks, priorities, and status-based filtering
- 📝 **Structured Logging**: Comprehensive logging with tracing

## Prerequisites

1. **MCP Todo Server**: Clone and run the MCP todo server from [mcp_todo_task](https://github.com/nimec77/mcp_todo_task)
2. **DeepSeek API Key**: Get an API key from [DeepSeek](https://platform.deepseek.com/)

## Installation

1. Clone this repository:
   ```bash
   git clone <your-repo-url>
   cd deepseek_mcp_tasks
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

## Configuration

Create a `.env` file in the project root with the following variables:

```env
# Required: DeepSeek API Configuration
DEEPSEEK_API_KEY=your_deepseek_api_key_here

# Optional: MCP Server Configuration
MCP_SERVER_COMMAND=./mcp_todo_task
MCP_SERVER_ARGS=

# Optional: Request Configuration
REQUEST_TIMEOUT=30
MAX_RETRIES=3
RETRY_DELAY=1000
```

Or export these as environment variables:
```bash
export DEEPSEEK_API_KEY="your_deepseek_api_key_here"
export MCP_SERVER_COMMAND="./mcp_todo_task"
export MCP_SERVER_ARGS=""
```

## Setup MCP Todo Server

1. Clone and setup the MCP todo server:
   ```bash
   git clone https://github.com/nimec77/mcp_todo_task.git
   cd mcp_todo_task
   cargo build --release
   ```

2. Run the server:
   ```bash
   ./target/release/mcp_todo_task
   ```

The server should start on `http://127.0.0.1:8000` by default.

## Usage

### Basic Commands

List all tasks:
```bash
./target/release/deepseek_mcp_tasks list
```

List tasks with a specific status:
```bash
./target/release/deepseek_mcp_tasks status pending
./target/release/deepseek_mcp_tasks status in_progress
./target/release/deepseek_mcp_tasks status completed
./target/release/deepseek_mcp_tasks status cancelled
```

Show task statistics:
```bash
./target/release/deepseek_mcp_tasks stats
```

Get available tools from MCP server:
```bash
./target/release/deepseek_mcp_tasks tools
```

### AI-Powered Analysis

Analyze pending tasks using DeepSeek AI:
```bash
./target/release/deepseek_mcp_tasks analyze
```

Analyze pending tasks using DeepSeek AI with MCP tools (recommended):
```bash
./target/release/deepseek_mcp_tasks analyze-with-tools
```

Enable verbose logging:
```bash
./target/release/deepseek_mcp_tasks -v list
```

### Command Options

#### `status` command:
- `<STATUS>`: The status to filter by (e.g., "pending", "in_progress", "completed", "cancelled")

#### Global options:
- `-v, --verbose`: Enable detailed logging output

## AI Analysis Features

The application provides two types of AI analysis:

### 1. Basic Analysis (`analyze`)
- Analyzes pending tasks using DeepSeek AI
- Provides priority assessment, complexity analysis, and recommendations
- Uses static task data provided to the AI

### 2. Tool-Enabled Analysis (`analyze-with-tools`)
- DeepSeek AI can interact with MCP tools in real-time
- Can query task details, create task breakdowns, and perform dynamic analysis
- Provides more comprehensive and up-to-date insights
- AI can access the full MCP server toolset for enhanced analysis

## Example Output

### Simple Task List
```
📋 All Tasks (5 total)
================================================================================
┌──────────┬────────────────────────────┬───────────┬──────────┬────────────┬────────────┬─────────┐
│    ID    │           Title            │  Status   │ Priority │  Due Date  │  Created   │  Tags   │
├──────────┼────────────────────────────┼───────────┼──────────┼────────────┼────────────┼─────────┤
│ abc12... │ Complete project setup     │ in_progre │   High   │ 2024-01-15 │ 2024-01-10 │ work    │
│ def34... │ Review code changes        │  pending  │  Medium  │ 2024-01-20 │ 2024-01-12 │ review  │
└──────────┴────────────────────────────┴───────────┴──────────┴────────────┴────────────┴─────────┘
```

### AI Analysis Output
```
🤖 Analyzing tasks with DeepSeek AI...

📊 DeepSeek Analysis Results:

**Priority Assessment:**
- High Priority: Task 1 (Complete project setup) - Due soon and critical for project launch
- Medium Priority: Task 2 (Review code changes) - Important but can be scheduled flexibly

**Complexity Analysis:**
- Simple: Task 2 (Review code changes) - Straightforward review process
- Moderate: Task 1 (Complete project setup) - Requires coordination and setup

**Actionable Recommendations:**
1. Focus on completing the project setup first due to its critical nature
2. Schedule code review after setup completion
3. Consider breaking down the setup task into smaller subtasks

**Risk Assessment:**
- Task 1 has moderate risk due to dependencies and coordination requirements
- Task 2 has low risk and can be easily rescheduled if needed
```

### Statistics View
```
📊 Task Summary
========================================
Total Tasks: 10
Unfinished Tasks: 5
Completion Rate: 50.0%

⚡ Priority Breakdown
==============================
🔴 High Priority: 2
🟡 Medium Priority: 2
⚪ No Priority Set: 1
```

## Architecture

The application is structured into several modules:

- **`config.rs`**: Configuration management with environment variables
- **`logger.rs`**: Centralized logging setup with tracing
- **`mcp_client.rs`**: MCP server communication client
- **`deepseek_client.rs`**: DeepSeek AI integration and analysis
- **`tooling.rs`**: MCP tool definitions and execution
- **`table_formatter.rs`**: Rich table formatting and display
- **`main.rs`**: CLI interface and application orchestration

## Error Handling

The application includes comprehensive error handling:

- **Configuration errors**: Clear messages for missing environment variables
- **API errors**: Detailed error messages for DeepSeek API issues
- **Network errors**: Retry logic with exponential backoff
- **Server errors**: Health checks and connection validation
- **Tool execution errors**: Graceful handling of MCP tool failures

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is open source and available under the [MIT License](LICENSE).

## Acknowledgments

- Based on [deepseek_mcp_iplocate](https://github.com/nimec77/deepseek_mcp_iplocate) project structure
- Uses [mcp_todo_task](https://github.com/nimec77/mcp_todo_task) MCP server
- Powered by [DeepSeek AI](https://platform.deepseek.com/) for intelligent task analysis
