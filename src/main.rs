//! Claude Code Stop Hook - AI-based Session Detector
//!
//! This hook uses an AI model (OpenAI-compatible API) to analyze Claude Code
//! transcripts and determine if the session should continue or stop.
//!
//! ## Configuration
//! All settings are read from a YAML config file.
//! Default path: ~/.claude/cc-goto-work/config.yaml

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process;
use std::time::Duration;

// ============================================================================
// Constants
// ============================================================================

/// Default config file path
const DEFAULT_CONFIG_PATH: &str = "~/.claude/cc-goto-work/config.yaml";
/// Read approximately last 10KB of transcript for efficiency
const TAIL_READ_BYTES: u64 = 10 * 1024;
/// Maximum number of transcript lines to send to AI
const AI_MAX_LINES: usize = 20;
/// Default API request timeout in seconds
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Parser, Debug)]
#[command(name = "cc-goto-work")]
#[command(about = "Claude Code Stop Hook - AI-based session detector")]
#[command(version)]
struct Args {
    /// Path to config file
    #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
    config: String,
}

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Deserialize)]
struct Config {
    /// OpenAI compatible API base URL
    api_base: String,
    /// API key for authentication
    api_key: String,
    /// Model name to use
    model: String,
    /// Request timeout in seconds (optional, default: 30)
    #[serde(default = "default_timeout")]
    timeout: u64,
    /// Custom system prompt (optional)
    #[serde(default)]
    system_prompt: Option<String>,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}

impl Config {
    fn load(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// Input received from Claude Code via stdin
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HookInput {
    session_id: Option<String>,
    transcript_path: Option<String>,
    cwd: Option<String>,
    hook_event_name: Option<String>,
    stop_hook_active: Option<bool>,
}

/// Output to control Claude's behavior
#[derive(Debug, Serialize)]
struct HookOutput {
    decision: String,
    reason: String,
}

/// A parsed line from the transcript
#[derive(Debug, Clone)]
struct TranscriptLine {
    #[allow(dead_code)]
    raw: String,
    json: Option<serde_json::Value>,
}

// ============================================================================
// AI Response
// ============================================================================

#[derive(Debug, Deserialize)]
struct AiCheckResponse {
    should_continue: bool,
    #[serde(default)]
    reason: String,
}

// ============================================================================
// Transcript Reading
// ============================================================================

fn read_transcript_tail(path: &PathBuf) -> Result<Vec<TranscriptLine>, Box<dyn std::error::Error>> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()),
    };

    let file_len = file.metadata()?.len();
    if file_len == 0 {
        return Ok(Vec::new());
    }

    let (start_pos, drop_first_line) = if file_len <= TAIL_READ_BYTES {
        (0, false)
    } else {
        (file_len - TAIL_READ_BYTES, true)
    };

    file.seek(SeekFrom::Start(start_pos))?;

    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut first_line = true;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if first_line && drop_first_line {
                    first_line = false;
                    continue;
                }
                first_line = false;

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let json = serde_json::from_str::<serde_json::Value>(trimmed).ok();
                lines.push(TranscriptLine {
                    raw: trimmed.to_string(),
                    json,
                });
            }
            Err(_) => break,
        }
    }

    Ok(lines)
}

// ============================================================================
// Transcript Formatting
// ============================================================================

fn format_transcript_for_ai(lines: &[TranscriptLine]) -> String {
    let recent_lines: Vec<_> = lines.iter().rev().take(AI_MAX_LINES).collect();
    let mut result = String::new();

    for line in recent_lines.into_iter().rev() {
        if let Some(json) = &line.json {
            let entry_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");

            match entry_type {
                "user" => {
                    if let Some(content) = json.pointer("/message/content").and_then(|v| v.as_str()) {
                        result.push_str(&format!("User: {}\n", content));
                    }
                }
                "assistant" => {
                    if let Some(content) = json.pointer("/message/content") {
                        let text = if let Some(s) = content.as_str() {
                            s.to_string()
                        } else if let Some(arr) = content.as_array() {
                            arr.iter()
                                .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n")
                        } else {
                            continue;
                        };
                        if !text.is_empty() {
                            result.push_str(&format!("Assistant: {}\n", text));
                        }
                    }
                    if let Some(stop_reason) = json.pointer("/message/stop_reason").and_then(|v| v.as_str()) {
                        result.push_str(&format!("[stop_reason: {}]\n", stop_reason));
                    }
                }
                "error" => {
                    let error_info = json.get("error").unwrap_or(json);
                    result.push_str(&format!("[Error: {}]\n", error_info));
                }
                _ => {}
            }
        }
    }

    result
}

// ============================================================================
// Default System Prompt
// ============================================================================

fn default_system_prompt() -> &'static str {
    r#"You are a supervisor monitoring a Claude Code session. Claude Code is an AI coding assistant that helps users with programming tasks through conversation.

Your task: Analyze the conversation transcript and determine if Claude stopped working prematurely or completed the task properly.

## Return "should_continue": true if:
- Claude's response was cut off mid-sentence or mid-code (output truncation)
- Claude encountered an API error, rate limit, or server issue
- Claude said "I will do X" but didn't actually do it
- Claude gave a partial answer and clearly has more work to do
- Claude's last message ends abruptly without conclusion
- There's an [Error: ...] in the transcript indicating a problem

## Return "should_continue": false if:
- Claude completed the requested task
- Claude asked the user a question and is waiting for response
- Claude explicitly said it's done or asked for feedback
- The conversation reached a natural stopping point
- Claude explained it cannot do something (legitimate refusal)
- User's request was fully addressed

## Output Format
You must respond with ONLY a JSON object, no other text:
{"should_continue": true, "reason": "brief explanation"}
or
{"should_continue": false, "reason": "brief explanation"}"#
}

// ============================================================================
// JSON Extraction
// ============================================================================

/// Remove common thinking/reasoning tags from model output
/// Handles: <think>...</think>, <thinking>...</thinking>, <reasoning>...</reasoning>, etc.
fn remove_thinking_tags(text: &str) -> String {
    let mut result = text.to_string();

    // Common thinking tag patterns
    let tag_patterns = [
        ("think", "think"),
        ("thinking", "thinking"),
        ("reasoning", "reasoning"),
        ("thought", "thought"),
        ("reflection", "reflection"),
    ];

    for (open, close) in tag_patterns {
        // Case-insensitive removal of <tag>...</tag>
        let open_tag = format!("<{}>", open);
        let close_tag = format!("</{}>", close);

        loop {
            let lower = result.to_lowercase();
            if let Some(start) = lower.find(&open_tag.to_lowercase()) {
                if let Some(end_offset) = lower[start..].find(&close_tag.to_lowercase()) {
                    let end = start + end_offset + close_tag.len();
                    result = format!("{}{}", &result[..start], &result[end..]);
                    continue;
                }
            }
            break;
        }
    }

    result.trim().to_string()
}

/// Extract JSON object from a string that may contain extra text
/// Looks for the last valid JSON object in the string
fn extract_json_from_response(text: &str) -> Option<&str> {
    // Try to find JSON object patterns from the end
    let mut depth = 0;
    let mut end_pos = None;
    let mut start_pos = None;

    // Scan from end to find the last complete JSON object
    for (i, c) in text.char_indices().rev() {
        match c {
            '}' => {
                if depth == 0 {
                    end_pos = Some(i + 1);
                }
                depth += 1;
            }
            '{' => {
                depth -= 1;
                if depth == 0 && end_pos.is_some() {
                    start_pos = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }

    match (start_pos, end_pos) {
        (Some(start), Some(end)) => Some(&text[start..end]),
        _ => None,
    }
}

/// Parse AI response, handling various output formats
fn parse_ai_response(content: &str) -> Option<AiCheckResponse> {
    // Step 1: Try direct parse (ideal case)
    if let Ok(response) = serde_json::from_str::<AiCheckResponse>(content) {
        return Some(response);
    }

    // Step 2: Remove thinking tags and try again
    let cleaned = remove_thinking_tags(content);
    if let Ok(response) = serde_json::from_str::<AiCheckResponse>(&cleaned) {
        return Some(response);
    }

    // Step 3: Extract JSON from cleaned content
    if let Some(json_str) = extract_json_from_response(&cleaned) {
        if let Ok(response) = serde_json::from_str::<AiCheckResponse>(json_str) {
            return Some(response);
        }
    }

    // Step 4: Try extracting from original content (in case tag removal broke something)
    if let Some(json_str) = extract_json_from_response(content) {
        if let Ok(response) = serde_json::from_str::<AiCheckResponse>(json_str) {
            return Some(response);
        }
    }

    None
}

// ============================================================================
// AI Check
// ============================================================================

async fn check_with_ai(lines: &[TranscriptLine], config: &Config) -> Option<(bool, String)> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: failed to create HTTP client: {}", e);
            return None;
        }
    };

    let transcript_text = format_transcript_for_ai(lines);
    if transcript_text.is_empty() {
        return None;
    }

    let system_prompt = config.system_prompt
        .as_deref()
        .unwrap_or_else(|| default_system_prompt());

    let request_body = serde_json::json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": transcript_text
            }
        ],
        "response_format": { "type": "json_object" },
        "max_tokens": 256,
        "temperature": 0
    });

    let url = format!("{}/chat/completions", config.api_base.trim_end_matches('/'));

    let response = match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: API request failed: {}", e);
            return None;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("Error: API returned status {}: {}", status, body);
        return None;
    }

    let body: serde_json::Value = match response.json().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: failed to parse API response: {}", e);
            return None;
        }
    };

    let content = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())?;

    // Parse response, handling various output formats (thinking tags, extra text, etc.)
    let decision = match parse_ai_response(content) {
        Some(d) => d,
        None => {
            eprintln!("Error: failed to parse AI response: {}", content);
            return None;
        }
    };

    let reason = if decision.reason.is_empty() {
        if decision.should_continue {
            "AI determined task is incomplete".to_string()
        } else {
            "AI determined task is complete".to_string()
        }
    } else {
        decision.reason
    };

    Some((decision.should_continue, reason))
}

// ============================================================================
// Path Expansion
// ============================================================================

fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") || path.starts_with("~\\") {
        if let Some(home) = dirs_next::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();

    if let Err(e) = run(&args).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // Load config
    let config_path = expand_path(&args.config);
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: failed to load config from {:?}: {}", config_path, e);
            eprintln!("Please create a config file at {} with the following format:", DEFAULT_CONFIG_PATH);
            eprintln!();
            eprintln!("api_base: https://api.openai.com/v1");
            eprintln!("api_key: your-api-key-here");
            eprintln!("model: gpt-4o-mini");
            eprintln!("timeout: 30  # optional");
            return Err(e);
        }
    };

    // Read input from stdin
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)?;

    let input: HookInput = serde_json::from_str(&input_str)?;

    // Get transcript path
    let transcript_path = match &input.transcript_path {
        Some(path) => expand_path(path),
        None => return Ok(()), // No transcript, allow stop
    };

    // Read transcript tail
    let lines = read_transcript_tail(&transcript_path)?;
    if lines.is_empty() {
        return Ok(());
    }

    // Check with AI
    match check_with_ai(&lines, &config).await {
        Some((true, reason)) => {
            // AI says continue
            let output = HookOutput {
                decision: "block".to_string(),
                reason: format!("AI: {}", reason),
            };
            println!("{}", serde_json::to_string(&output)?);
        }
        Some((false, _)) => {
            // AI says stop is fine - do nothing
        }
        None => {
            // AI check failed - allow stop by default
            eprintln!("Warning: AI check failed, allowing stop");
        }
    }

    Ok(())
}
