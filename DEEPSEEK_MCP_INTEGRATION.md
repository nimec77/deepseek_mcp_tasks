# DeepSeek-MCP Integration

This document explains the integration between DeepSeek models and MCP (Model Context Protocol) servers via tooling in this Rust application.

## Overview

The application creates a bridge between DeepSeek's AI models and MCP servers, allowing DeepSeek to directly invoke tools and functions available on MCP servers. This enables intelligent task analysis with real-time data access.

## Architecture

### Components

1. **DeepSeekClient** (`src/deepseek_client.rs`)
   - Primary client for DeepSeek API interactions
   - Supports both simple chat and tool-enabled chat
   - Manages conversation flow with tool calls

2. **Tooling Module** (`src/tooling.rs`)
   - Custom DeepSeek API client with tool support
   - Tool definition creators for MCP integration
   - Tool execution handlers

3. **MCP Client** (`src/mcp_client.rs`)
   - Interface to MCP servers
   - Task management operations
   - Tool discovery and execution

## Key Features

### Tool Integration

The integration provides several types of tools that DeepSeek can use:

1. **Generic MCP Invoke Tool**: 
   ```rust
   mcp_invoke_tool() -> ToolObject
   ```
   - Allows DeepSeek to call any MCP server tool
   - Parameters: server, tool, arguments

2. **Specific MCP Tools**:
   - Automatically generated from MCP server capabilities
   - Named with `mcp_` prefix (e.g., `mcp_list_tasks`)

3. **Task Manager Tool**:
   ```rust
   create_task_analysis_tool() -> ToolObject
   ```
   - Specialized tool for task operations
   - Actions: list_all, list_pending, list_by_status, analyze_priorities

### Usage Examples

#### Basic Analysis (without tools)
```bash
cargo run -- analyze
```

#### Advanced Analysis (with MCP tools)
```bash
cargo run -- analyze-with-tools
```

The tool-enabled analysis allows DeepSeek to:
- Query the MCP server for real-time task data
- Perform dynamic analysis based on current state
- Get additional task details as needed
- Make data-driven recommendations

## Implementation Details

### Tool Definition Format

Tools are defined using the DeepSeek API format:

```rust
ToolObject {
    tool_type: "function".to_string(),
    function: Function {
        name: "tool_name".to_string(),
        description: "Tool description".to_string(),
        parameters: json!({
            "type": "object",
            "properties": { /* ... */ },
            "required": [/* ... */]
        }),
    },
}
```

### Tool Execution Flow

1. DeepSeek receives user message with available tools
2. AI decides to call a tool based on context
3. Tool call is routed to appropriate MCP server
4. MCP server executes the tool and returns results
5. Results are fed back to DeepSeek for final response
6. Process repeats up to 5 iterations if needed

### MCP Tool Mapping

The system automatically creates DeepSeek-compatible tools from MCP server capabilities:

- MCP tool `list_tasks` → DeepSeek tool `mcp_list_tasks`
- MCP tool `create-task` → DeepSeek tool `mcp_create_task`
- Generic invocation via `mcp_invoke` tool

## Configuration

### Environment Variables

Required:
- `DEEPSEEK_API_KEY`: Your DeepSeek API key
- `MCP_SERVER_COMMAND`: MCP server executable (default: `./mcp_todo_task`)

Optional:
- `MCP_SERVER_ARGS`: Additional arguments for MCP server

### API Settings

DeepSeek API calls are configured with:
- Model: `deepseek-chat`
- Temperature: 0.7
- Max tokens: 4000
- Tool choice: "auto" (AI decides when to use tools)

## Error Handling

The integration includes robust error handling:

- API key validation
- MCP server connectivity checks
- Tool execution error recovery
- Timeout and retry logic
- Graceful degradation when tools fail

## Benefits

### Enhanced Analysis
- **Real-time data**: DeepSeek accesses current task states
- **Dynamic queries**: AI can explore data as needed
- **Contextual insights**: Analysis based on live MCP data

### Flexible Integration
- **Server-agnostic**: Works with any MCP-compatible server
- **Tool discovery**: Automatically adapts to available tools
- **Extensible**: Easy to add new tool types

### Intelligent Automation
- **Conditional logic**: AI decides when tools are needed
- **Multi-step workflows**: Chains multiple tool calls
- **Adaptive responses**: Varies analysis based on data

## Example Output

When running `cargo run -- analyze-with-tools`, you might see:

```
📋 Found 3 pending tasks:
  1. Implement user authentication (Status: pending)
  2. Design database schema (Status: pending)
  3. Set up CI/CD pipeline (Status: pending)

🚀 Analyzing tasks with DeepSeek AI using MCP tools...
📡 The AI can now query the MCP server directly for real-time task data!

🔧 DeepSeek Analysis with MCP Tools:

[AI uses task_manager tool to get priority breakdown]
[AI uses mcp_list_tasks to get detailed task information]
[AI provides comprehensive analysis with real-time data]
```

## Future Enhancements

Potential improvements:
- Multiple MCP server support
- Custom tool definitions via config
- Caching for frequently used tool results
- Advanced error recovery strategies
- Integration with more DeepSeek model variants

## Troubleshooting

Common issues and solutions:

1. **API Key Issues**: Ensure `DEEPSEEK_API_KEY` is set correctly
2. **MCP Server Not Found**: Check `MCP_SERVER_COMMAND` path
3. **Tool Execution Failures**: Verify MCP server is running and responsive
4. **Rate Limiting**: DeepSeek API has usage limits - consider adding delays

For more details, check the source code documentation and error messages.
