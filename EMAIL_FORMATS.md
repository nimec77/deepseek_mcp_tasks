# Email-Friendly Report Formats

The `analyze-with-tools` command now supports saving reports in multiple formats that are convenient for sharing via email:

## Supported Formats

### 📧 Markdown (.md) - Recommended for Email
**Best for**: Email attachments, GitHub issues, Slack messages, documentation

```bash
cargo run -- analyze-with-tools -o report.md
```

Features:
- Clean formatting with headers and sections
- Easy to read in plain text
- Compatible with most modern platforms
- Perfect for copying into email bodies

### 📋 Plain Text (.txt) - Universal Compatibility
**Best for**: Legacy email systems, maximum compatibility

```bash
cargo run -- analyze-with-tools -o report.txt
```

Features:
- Works in any email client
- No special formatting characters
- Easy to copy and paste
- Clear ASCII separators

### 🔧 JSON (.json) - Structured Data
**Best for**: Data processing, archival, API integration

```bash
cargo run -- analyze-with-tools -o report.json
```

Features:
- Complete structured data
- Machine-readable format
- Includes all metadata
- Perfect for further processing

## Example Usage

```bash
# Email-friendly markdown report
cargo run -- analyze-with-tools -o reports/team_analysis_$(date +%Y%m%d).md

# Universal compatibility text report
cargo run -- analyze-with-tools -o reports/analysis.txt

# Structured JSON report
cargo run -- analyze-with-tools -o reports/analysis_data.json
```

## Email Sharing Tips

### For Markdown Reports
- Attach `.md` files to emails
- Most email clients will display them properly
- Can be copied directly into email body for inline viewing

### For Plain Text Reports
- Perfect for inline email content
- Copy and paste directly into email body
- Works with any email client

### For JSON Reports
- Best as email attachments
- Include a summary in the email body
- Great for technical audiences who need raw data

## Report Contents

All formats include:
- ✅ Task summary with details
- ✅ AI analysis and recommendations 
- ✅ Priority assessments
- ✅ Risk analysis
- ✅ Execution timeline
- ✅ Generation metadata
- ✅ Tool usage statistics
