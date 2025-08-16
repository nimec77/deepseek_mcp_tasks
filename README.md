# MCP Tasks

A Rust application that integrates with an MCP (Model Context Protocol) todo server to display and manage tasks in a beautiful table format.

## Features

- 🚀 **MCP Integration**: Connects to local MCP todo task server
- 📊 **Rich Tables**: Beautiful table formatting with task details
- ⚡ **Fast & Efficient**: Built with async Rust for performance
- 📈 **Statistics**: Comprehensive task statistics and breakdowns
- 🎯 **Filtering**: Support for overdue tasks, priorities, and more

## Prerequisites

1. **MCP Todo Server**: Clone and run the MCP todo server from [mcp_todo_task](https://github.com/nimec77/mcp_todo_task)

## Installation

1. Clone this repository:
   ```bash
   git clone <your-repo-url>
   cd mcp_tasks
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

## Configuration

Create a `.env` file in the project root with the following variables:

```env
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

List all unfinished tasks:
```bash
./target/release/mcp-tasks list
```


Show detailed statistics:
```bash
./target/release/mcp-tasks list --detailed
```

Show only overdue tasks:
```bash
./target/release/mcp-tasks list --overdue-only
```

Check MCP server health:
```bash
./target/release/mcp-tasks health
```

Show task statistics:
```bash
./target/release/mcp-tasks stats
```

Enable verbose logging:
```bash
./target/release/mcp-tasks -v list
```

### Command Options

#### `list` command:
- `--detailed`: Show comprehensive task statistics and breakdowns
- `--overdue-only`: Display only tasks that are past their due date

#### Global options:
- `-v, --verbose`: Enable detailed logging output

## Example Output

### Simple Task List
```
🎯 Unfinished Tasks (5 total)
================================================================================
┌──────────┬────────────────────────────┬───────────┬──────────┬────────────┬────────────┬─────────┐
│    ID    │           Title            │  Status   │ Priority │  Due Date  │  Created   │  Tags   │
├──────────┼────────────────────────────┼───────────┼──────────┼────────────┼────────────┼─────────┤
│ abc12... │ Complete project setup     │ in_progre │   High   │ 2024-01-15 │ 2024-01-10 │ work    │
│ def34... │ Review code changes        │  pending  │  Medium  │ 2024-01-20 │ 2024-01-12 │ review  │
└──────────┴────────────────────────────┴───────────┴──────────┴────────────┴────────────┴─────────┘
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
- **`table_formatter.rs`**: Rich table formatting and display
- **`main.rs`**: CLI interface and application orchestration

## Error Handling

The application includes comprehensive error handling:

- **Configuration errors**: Clear messages for missing environment variables
- **Network errors**: Retry logic with exponential backoff
- **Server errors**: Health checks and connection validation

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
