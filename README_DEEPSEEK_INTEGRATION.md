# DeepSeek AI Integration for MCP Tasks

This document describes the new DeepSeek AI integration feature that has been added to the MCP Tasks application.

## Overview

The application now includes a new `analyze` command that:

1. **Fetches pending tasks** from the MCP server using the existing MCP client
2. **Sends these tasks to DeepSeek AI** for intelligent analysis
3. **Provides comprehensive insights** about task prioritization, complexity, and recommendations

## New Features Added

### 1. DeepSeek Client Module (`src/deepseek_client.rs`)

- **`DeepSeekClient`** struct that wraps the genai library for DeepSeek API access
- **`analyze_tasks()`** method that formats tasks and sends them to DeepSeek for analysis
- **Intelligent prompt engineering** that requests specific analysis categories:
  - Priority Assessment
  - Complexity Analysis  
  - Dependency Mapping
  - Actionable Recommendations
  - Risk Assessment

### 2. Enhanced Configuration (`src/config.rs`)

- **`deepseek_api_key`** field added to the Config struct
- **Environment variable support** for `DEEPSEEK_API_KEY`
- **Updated environment template** with DeepSeek API key configuration

### 3. New Analyze Command (`src/main.rs`)

- **`Commands::Analyze`** enum variant for the new command
- **`handle_analyze_command()`** function that orchestrates the analysis workflow
- **Rich console output** with emojis and formatted results
- **Comprehensive error handling** with helpful error messages

### 4. Updated Dependencies

- **`genai = "0.3.5"`** - Unified API for multiple AI providers including DeepSeek
- **DeepSeek support** through OpenAI-compatible interface

## Usage

### 1. Set up your DeepSeek API Key

```bash
# Add to your .env file
echo "DEEPSEEK_API_KEY=your_api_key_here" >> .env

# Or export in your shell
export DEEPSEEK_API_KEY=your_api_key_here
```

### 2. Run the analyze command

```bash
# Analyze all pending tasks with DeepSeek AI
cargo run -- analyze

# Enable verbose logging for debugging
cargo run -- --verbose analyze
```

### 3. Example Output

```
📋 Found 3 pending tasks:
  1. Implement user authentication (Status: pending)
     Priority: high
     Due: 2024-01-15
  2. Create API documentation (Status: pending)
  3. Set up CI/CD pipeline (Status: pending)
     Priority: medium

🤖 Analyzing tasks with DeepSeek AI...

📊 DeepSeek Analysis Results:

## Priority Assessment
Based on the provided tasks, I recommend prioritizing them as follows:

1. **High Priority: Implement user authentication**
   - Critical for application security
   - Has a defined due date (2024-01-15)
   - Likely blocks other development work

2. **Medium Priority: Set up CI/CD pipeline**
   - Important for development efficiency
   - Should be implemented before major feature releases

3. **Lower Priority: Create API documentation**
   - Important for maintainability but can be done incrementally

## Complexity Analysis
- **Simple**: Create API documentation (can be done incrementally)
- **Moderate**: Set up CI/CD pipeline (requires configuration but well-documented)
- **Complex**: Implement user authentication (requires security considerations)

## Actionable Recommendations
1. Start with user authentication implementation immediately due to due date
2. Parallel work on CI/CD setup by different team member if available
3. API documentation can be created alongside feature development

## Risk Assessment
- User authentication task is at risk due to tight deadline
- Consider breaking down complex tasks into smaller, manageable subtasks
```

## Technical Implementation Details

### DeepSeek Integration Architecture

```rust
// The DeepSeek client uses the genai library for unified API access
pub struct DeepSeekClient {
    client: Client,           // genai::Client for API communication
    model: String,           // "deepseek-chat" model identifier
}

// Main analysis workflow
pub async fn analyze_tasks(&self, tasks: Vec<Task>) -> Result<String> {
    // 1. Format tasks for analysis
    let task_summary = self.format_tasks_for_analysis(&tasks);
    
    // 2. Create structured analysis prompt
    let analysis_prompt = self.create_analysis_prompt(&task_summary, tasks.len());
    
    // 3. Send to DeepSeek via genai library
    let chat_req = ChatRequest::new(vec![
        ChatMessage::system("You are a task analysis expert..."),
        ChatMessage::user(analysis_prompt),
    ]);
    
    // 4. Process and return results
    let chat_res = self.client.exec_chat(&self.model, chat_req, None).await?;
    Ok(chat_res.content_text_as_str()?.to_string())
}
```

### Error Handling

The integration includes comprehensive error handling for common issues:

- **Missing API Key**: Clear instructions on how to set the DEEPSEEK_API_KEY
- **API Failures**: Helpful troubleshooting steps for API connectivity issues
- **No Pending Tasks**: Graceful handling when no tasks need analysis
- **MCP Server Issues**: Separation of MCP vs DeepSeek-related errors

### Integration with Existing Architecture

The new functionality seamlessly integrates with the existing MCP Tasks architecture:

- **Uses existing MCP client** for fetching tasks
- **Leverages existing configuration system** for API key management
- **Follows existing command pattern** in main.rs
- **Uses existing logging infrastructure** with tracing

## Benefits

1. **Intelligent Task Prioritization**: AI-powered insights help identify which tasks to tackle first
2. **Complexity Assessment**: Understand the relative difficulty of different tasks
3. **Risk Identification**: Spot potential issues before they become problems
4. **Actionable Recommendations**: Get specific guidance on how to approach your workload
5. **Dependency Analysis**: Understand relationships between different tasks

## Future Enhancements

Potential improvements for future versions:

1. **Custom Analysis Types**: Allow users to specify different analysis focuses
2. **Historical Analysis**: Track how AI recommendations performed over time
3. **Integration with Other AI Providers**: Support for Claude, GPT-4, etc.
4. **Export Options**: Save analysis results to files or external systems
5. **Interactive Mode**: Allow follow-up questions about the analysis

## Troubleshooting

### Common Issues

1. **"DEEPSEEK_API_KEY environment variable is not set"**
   - Solution: Set the API key in your environment or .env file

2. **"Failed to analyze tasks: API error"**
   - Check your API key is valid
   - Verify you have sufficient API credits
   - Ensure internet connectivity

3. **"No pending tasks found to analyze"**
   - This is normal if you have no tasks with "pending" status
   - Try creating some pending tasks in your MCP server first

### Getting Help

If you encounter issues:

1. Run with `--verbose` flag for detailed logging
2. Check the MCP server is running and accessible
3. Verify your DeepSeek API key and credits
4. Check the GitHub repository for known issues and solutions
