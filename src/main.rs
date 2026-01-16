//! Claude Code Stop Hook - RESOURCE_EXHAUSTED Detector
//!
//! This hook detects if Claude stopped due to a RESOURCE_EXHAUSTED error
//! and instructs it to continue working if so.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::Duration;

const DEFAULT_WAIT_SECONDS: u64 = 30;

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

/// A single entry in the transcript JSONL file
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TranscriptEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    message: Option<TranscriptMessage>,
}

#[derive(Debug, Deserialize)]
struct TranscriptMessage {
    content: Option<serde_json::Value>,
    stop_reason: Option<String>,
}

/// Parse command line arguments to get wait time
fn parse_args() -> u64 {
    let args: Vec<String> = std::env::args().collect();
    let mut wait_seconds = DEFAULT_WAIT_SECONDS;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--wait" | "-w" => {
                if i + 1 < args.len() {
                    if let Ok(secs) = args[i + 1].parse::<u64>() {
                        wait_seconds = secs;
                    }
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("cc-goto-work - Claude Code RESOURCE_EXHAUSTED Hook");
                println!();
                println!("USAGE:");
                println!("    cc-goto-work [OPTIONS]");
                println!();
                println!("OPTIONS:");
                println!("    -w, --wait <SECONDS>    Wait time before continuing (default: {})", DEFAULT_WAIT_SECONDS);
                println!("    -h, --help              Print help information");
                println!("    -V, --version           Print version information");
                process::exit(0);
            }
            "--version" | "-V" => {
                println!("cc-goto-work {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    wait_seconds
}

fn main() {
    let wait_seconds = parse_args();

    if let Err(e) = run(wait_seconds) {
        eprintln!("Hook error: {}", e);
        process::exit(1);
    }
}

fn run(wait_seconds: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Read input from stdin
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)?;

    let input: HookInput = serde_json::from_str(&input_str)?;

    // Prevent infinite loops - if stop_hook_active is true, allow stop
    if input.stop_hook_active.unwrap_or(false) {
        // Exit 0 without output = allow stop
        return Ok(());
    }

    // Get transcript path
    let transcript_path = match &input.transcript_path {
        Some(path) => expand_path(path),
        None => {
            // No transcript path, allow stop
            return Ok(());
        }
    };

    // Check if RESOURCE_EXHAUSTED caused the stop
    if is_resource_exhausted(&transcript_path)? {
        // Wait before continuing to avoid rapid retries
        if wait_seconds > 0 {
            thread::sleep(Duration::from_secs(wait_seconds));
        }

        // Block the stop and tell Claude to continue
        let output = HookOutput {
            decision: "block".to_string(),
            reason: "Detected RESOURCE_EXHAUSTED error. Please continue working on the task."
                .to_string(),
        };
        println!("{}", serde_json::to_string(&output)?);
    }

    // Exit 0 - if we printed JSON, it will be processed; otherwise stop is allowed
    Ok(())
}

/// Expand ~ to home directory
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") || path.starts_with("~\\") {
        if let Some(home) = dirs_next::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Check if the transcript indicates a RESOURCE_EXHAUSTED error
fn is_resource_exhausted(transcript_path: &PathBuf) -> Result<bool, Box<dyn std::error::Error>> {
    let file = match File::open(transcript_path) {
        Ok(f) => f,
        Err(_) => return Ok(false), // File doesn't exist, allow stop
    };

    let reader = BufReader::new(file);
    let mut last_lines: Vec<String> = Vec::new();

    // Read all lines and keep the last few for analysis
    for line in reader.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            last_lines.push(line);
            // Keep only last 10 lines to check
            if last_lines.len() > 10 {
                last_lines.remove(0);
            }
        }
    }

    // Check each of the last lines for RESOURCE_EXHAUSTED
    for line in &last_lines {
        if line.contains("RESOURCE_EXHAUSTED") {
            return Ok(true);
        }

        // Also try to parse as JSON and check content
        if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
            // Check stop_reason
            if let Some(ref msg) = entry.message {
                if let Some(ref reason) = msg.stop_reason {
                    if reason.contains("RESOURCE_EXHAUSTED") {
                        return Ok(true);
                    }
                }

                // Check content for error messages
                if let Some(ref content) = msg.content {
                    let content_str = content.to_string();
                    if content_str.contains("RESOURCE_EXHAUSTED") {
                        return Ok(true);
                    }
                }
            }
        }

        // Check for raw JSON containing the error
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            if contains_resource_exhausted(&value) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Recursively check if a JSON value contains RESOURCE_EXHAUSTED
fn contains_resource_exhausted(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(s) => s.contains("RESOURCE_EXHAUSTED"),
        serde_json::Value::Array(arr) => arr.iter().any(contains_resource_exhausted),
        serde_json::Value::Object(obj) => obj.values().any(contains_resource_exhausted),
        _ => false,
    }
}
