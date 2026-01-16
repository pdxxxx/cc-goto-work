//! Claude Code Stop Hook - Interruption Detector
//!
//! This hook detects common interruption causes from the Claude Code transcript JSONL
//! and instructs Claude to continue when appropriate.
//!
//! ## Detection Priority (first match wins)
//! 1. Fatal errors (context exceeded, cost limit) → Allow stop (no retry)
//! 2. stop_reason boundary (max_tokens → Block, end_turn → Allow)
//! 3. Structured error type (last 5 lines only) → Block + Wait
//! 4. HTTP status codes (last 5 lines only) → Block + Wait
//! 5. Raw text fallback (last 8 lines only) → Block + Wait

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::Duration;

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_WAIT_SECONDS: u64 = 30;
/// Read approximately last 10KB of transcript for efficiency
const TAIL_READ_BYTES: u64 = 10 * 1024;
/// Only check last N lines for structured error detection to avoid false positives from old errors
const RECENT_ERROR_LINES: usize = 5;
/// Only check last N lines for raw text fallback to reduce false positives
const RAW_FALLBACK_LINES: usize = 8;

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
    /// Indicates if a previous stop hook already intervened in this session
    stop_hook_active: Option<bool>,
}

/// Output to control Claude's behavior
#[derive(Debug, Serialize)]
struct HookOutput {
    decision: String,
    reason: String,
}

/// Internal action to take after detection
#[derive(Debug)]
struct HookAction {
    wait_seconds: u64,
    output: HookOutput,
}

/// A parsed line from the transcript
#[derive(Debug, Clone)]
struct TranscriptLine {
    raw: String,
    json: Option<serde_json::Value>,
}

// ============================================================================
// Stop Cause Classification
// ============================================================================

/// Represents the detected cause of interruption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopCause {
    // Retryable causes (Block + Wait)
    /// Output truncated due to max_tokens limit
    MaxTokens,
    /// API resource exhausted (quota/overload)
    ResourceExhausted,
    /// HTTP 429 rate limit
    RateLimited,
    /// Server overload (HTTP 503/529)
    Overloaded,
    /// Network or service unavailable
    Unavailable,

    // Non-retryable causes (Allow stop)
    /// Context window exceeded - retrying won't help
    ContextLengthExceeded,
    /// Cost/spending limit reached - must not retry
    CostLimitReached,
}

impl StopCause {
    /// Returns true if this error is transient and can be retried
    fn is_retryable(self) -> bool {
        matches!(
            self,
            StopCause::MaxTokens
                | StopCause::ResourceExhausted
                | StopCause::RateLimited
                | StopCause::Overloaded
                | StopCause::Unavailable
        )
    }

    /// Returns the wait time before retrying (0 for max_tokens, configured for others)
    fn wait_seconds(self, configured: u64) -> u64 {
        match self {
            StopCause::MaxTokens => 0, // No wait needed, just continue output
            _ if self.is_retryable() => configured,
            _ => 0,
        }
    }

    /// Returns a human-readable reason for the hook decision
    fn reason(self) -> &'static str {
        match self {
            StopCause::MaxTokens => {
                "Detected stop_reason=max_tokens. Please continue output from where you left off."
            }
            StopCause::ResourceExhausted => {
                "Detected retryable API error (RESOURCE_EXHAUSTED). Please continue working."
            }
            StopCause::RateLimited => {
                "Detected API rate limit (HTTP 429). Please continue working after wait."
            }
            StopCause::Overloaded => {
                "Detected server overload (HTTP 503/529). Please continue working after wait."
            }
            StopCause::Unavailable => {
                "Detected network/service unavailability. Please continue working after wait."
            }
            StopCause::ContextLengthExceeded => {
                "Context length exceeded. Cannot retry - please use /compact to reduce context."
            }
            StopCause::CostLimitReached => {
                "Cost/spending limit reached. Cannot retry - please check your budget settings."
            }
        }
    }
}

// ============================================================================
// Detection Outcome
// ============================================================================

/// Result of running a detector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectionOutcome {
    /// Allow the stop (do not block)
    Allow,
    /// Block the stop and retry with given cause
    Block(StopCause),
    /// No match, continue to next detector
    NoMatch,
}

// ============================================================================
// Detector Functions
// ============================================================================

/// Detector function type
type DetectorFn = fn(&[TranscriptLine], bool) -> DetectionOutcome;

/// Ordered list of detectors (priority order)
const DETECTORS: &[DetectorFn] = &[
    detect_fatal_errors,        // Must be first to prevent infinite loops
    detect_stop_reason_boundary,
    detect_structured_error,
    detect_http_status,
    detect_raw_fallback,
];

/// Detect fatal errors that should NEVER be retried
fn detect_fatal_errors(lines: &[TranscriptLine], _stop_hook_active: bool) -> DetectionOutcome {
    for line in lines.iter().rev() {
        // Check structured JSON first
        if let Some(json) = &line.json {
            if let Some(cause) = classify_fatal_error_json(json) {
                return DetectionOutcome::Block(cause);
            }
        }
        // Check raw text for fatal patterns
        if let Some(cause) = classify_fatal_error_raw(&line.raw) {
            return DetectionOutcome::Block(cause);
        }
    }
    DetectionOutcome::NoMatch
}

fn classify_fatal_error_json(value: &serde_json::Value) -> Option<StopCause> {
    // Check error.type field
    if let Some(error_type) = value.pointer("/error/type").and_then(|v| v.as_str()) {
        let t = error_type.to_ascii_lowercase();
        if t.contains("context_length") || t.contains("context_window") {
            return Some(StopCause::ContextLengthExceeded);
        }
    }
    // Check error.message for cost limit
    if let Some(msg) = value.pointer("/error/message").and_then(|v| v.as_str()) {
        let m = msg.to_ascii_lowercase();
        if m.contains("cost limit") || m.contains("spending limit") || m.contains("budget") {
            return Some(StopCause::CostLimitReached);
        }
    }
    None
}

fn classify_fatal_error_raw(raw: &str) -> Option<StopCause> {
    let upper = raw.to_ascii_uppercase();
    if upper.contains("CONTEXT_LENGTH_EXCEEDED") || upper.contains("CONTEXT_WINDOW_EXCEEDED") {
        return Some(StopCause::ContextLengthExceeded);
    }
    if upper.contains("COST_LIMIT") || upper.contains("SPENDING_LIMIT")
        || raw.to_ascii_lowercase().contains("budget exceeded") {
        return Some(StopCause::CostLimitReached);
    }
    None
}

/// Detect stop_reason boundary - establishes if this is a normal stop
fn detect_stop_reason_boundary(lines: &[TranscriptLine], _stop_hook_active: bool) -> DetectionOutcome {
    for line in lines.iter().rev() {
        let json = match &line.json {
            Some(v) => v,
            None => continue,
        };

        let stop_reason = match extract_stop_reason(json) {
            Some(sr) => sr,
            None => continue,
        };

        // max_tokens means output was truncated - should continue
        if stop_reason.eq_ignore_ascii_case("max_tokens") {
            return DetectionOutcome::Block(StopCause::MaxTokens);
        }
        // end_turn means normal completion - allow stop, don't check old errors
        if stop_reason.eq_ignore_ascii_case("end_turn") || stop_reason.eq_ignore_ascii_case("stop_sequence") {
            return DetectionOutcome::Allow;
        }
        // error means continue checking for specific error types
        if stop_reason.eq_ignore_ascii_case("error") {
            return DetectionOutcome::NoMatch;
        }
        // Unknown stop_reason - allow by default
        return DetectionOutcome::Allow;
    }
    DetectionOutcome::NoMatch
}

fn extract_stop_reason(value: &serde_json::Value) -> Option<&str> {
    value
        .pointer("/message/stop_reason")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("stop_reason").and_then(|v| v.as_str()))
}

/// Detect structured error type in JSON
fn detect_structured_error(lines: &[TranscriptLine], _stop_hook_active: bool) -> DetectionOutcome {
    // Only check the last N lines to avoid triggering on old historical errors
    let start = lines.len().saturating_sub(RECENT_ERROR_LINES);
    for line in lines[start..].iter().rev() {
        let json = match &line.json {
            Some(v) => v,
            None => continue,
        };

        // Only process error-type entries
        let entry_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !entry_type.eq_ignore_ascii_case("error") {
            continue;
        }

        // Check error.type field
        if let Some(error_type) = json.pointer("/error/type").and_then(|v| v.as_str()) {
            if let Some(cause) = classify_error_type(error_type) {
                return DetectionOutcome::Block(cause);
            }
        }

        // Also check error.message for known patterns
        if let Some(error_msg) = json.pointer("/error/message").and_then(|v| v.as_str()) {
            if let Some(cause) = classify_error_message(error_msg) {
                return DetectionOutcome::Block(cause);
            }
        }
    }
    DetectionOutcome::NoMatch
}

fn classify_error_type(error_type: &str) -> Option<StopCause> {
    let t = error_type.to_ascii_uppercase();
    if t.contains("RESOURCE_EXHAUSTED") {
        return Some(StopCause::ResourceExhausted);
    }
    if t.contains("RATE_LIMIT") || t.contains("TOO_MANY_REQUESTS") {
        return Some(StopCause::RateLimited);
    }
    if t.contains("OVERLOADED") || t.contains("OVERLOAD") {
        return Some(StopCause::Overloaded);
    }
    if t.contains("UNAVAILABLE") {
        return Some(StopCause::Unavailable);
    }
    None
}

fn classify_error_message(msg: &str) -> Option<StopCause> {
    let m = msg.to_ascii_uppercase();
    if m.contains("RESOURCE_EXHAUSTED") {
        return Some(StopCause::ResourceExhausted);
    }
    if m.contains("RATE LIMIT") || m.contains("TOO MANY REQUESTS") {
        return Some(StopCause::RateLimited);
    }
    if m.contains("OVERLOADED") {
        return Some(StopCause::Overloaded);
    }
    None
}

/// Detect HTTP status codes indicating transient errors
fn detect_http_status(lines: &[TranscriptLine], _stop_hook_active: bool) -> DetectionOutcome {
    // Only check the last N lines to avoid triggering on old historical errors
    let start = lines.len().saturating_sub(RECENT_ERROR_LINES);
    for line in lines[start..].iter().rev() {
        let json = match &line.json {
            Some(v) => v,
            None => continue,
        };

        // Only check error-like entries
        let is_error = json
            .get("type")
            .and_then(|v| v.as_str())
            .map(|t| t.eq_ignore_ascii_case("error"))
            .unwrap_or(false)
            || json.get("error").is_some();

        if !is_error {
            continue;
        }

        if let Some(status) = extract_http_status(json) {
            match status {
                429 => return DetectionOutcome::Block(StopCause::RateLimited),
                503 | 529 => return DetectionOutcome::Block(StopCause::Overloaded),
                _ => {}
            }
        }
    }
    DetectionOutcome::NoMatch
}

fn extract_http_status(value: &serde_json::Value) -> Option<i64> {
    // Check common status field names
    for key in &["status", "status_code", "http_status", "statusCode"] {
        if let Some(status) = value.get(*key).and_then(|v| v.as_i64()) {
            return Some(status);
        }
        // Also check inside error object
        if let Some(status) = value.pointer(&format!("/error/{}", key)).and_then(|v| v.as_i64()) {
            return Some(status);
        }
    }
    None
}

/// Raw text fallback for when JSON parsing fails
fn detect_raw_fallback(lines: &[TranscriptLine], _stop_hook_active: bool) -> DetectionOutcome {
    // Only check the last N lines to reduce false positives
    let start = lines.len().saturating_sub(RAW_FALLBACK_LINES);
    for line in lines[start..].iter().rev() {
        // Skip if JSON was successfully parsed (already handled)
        if line.json.is_some() {
            continue;
        }
        if let Some(cause) = classify_raw_text(&line.raw) {
            return DetectionOutcome::Block(cause);
        }
    }
    DetectionOutcome::NoMatch
}

fn classify_raw_text(raw: &str) -> Option<StopCause> {
    let upper = raw.to_ascii_uppercase();

    // Check for retryable errors
    if upper.contains("RESOURCE_EXHAUSTED") {
        return Some(StopCause::ResourceExhausted);
    }
    if upper.contains("OVERLOADED") {
        return Some(StopCause::Overloaded);
    }
    if upper.contains("UNAVAILABLE") {
        return Some(StopCause::Unavailable);
    }

    // Check for HTTP status patterns
    if upper.contains("HTTP 429") || raw.contains("\"status\":429") || raw.contains("\"status\": 429") {
        return Some(StopCause::RateLimited);
    }
    if upper.contains("HTTP 503") || upper.contains("HTTP 529")
        || raw.contains("\"status\":503") || raw.contains("\"status\":529") {
        return Some(StopCause::Overloaded);
    }

    None
}

// ============================================================================
// Transcript Reading
// ============================================================================

/// Read the tail of the transcript file efficiently using Seek
fn read_transcript_tail(path: &PathBuf) -> Result<Vec<TranscriptLine>, Box<dyn std::error::Error>> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()),
    };

    let file_len = file.metadata()?.len();
    if file_len == 0 {
        return Ok(Vec::new());
    }

    // Determine read position
    let (start_pos, drop_first_line) = if file_len <= TAIL_READ_BYTES {
        // File is small enough to read entirely
        (0, false)
    } else {
        // Seek to near the end
        let start = file_len - TAIL_READ_BYTES;
        // We'll drop the first line since it's likely partial
        (start, true)
    };

    file.seek(SeekFrom::Start(start_pos))?;

    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut first_line = true;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                // Skip first line if we started mid-file (likely partial)
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
// Core Logic
// ============================================================================

/// Run all detectors and determine the action to take
fn detect(lines: &[TranscriptLine], stop_hook_active: bool) -> DetectionOutcome {
    for detector in DETECTORS {
        let outcome = detector(lines, stop_hook_active);
        if outcome != DetectionOutcome::NoMatch {
            return outcome;
        }
    }
    DetectionOutcome::NoMatch
}

/// Decide what action to take based on detection outcome
fn decide_action(
    lines: &[TranscriptLine],
    stop_hook_active: bool,
    wait_seconds: u64,
) -> Option<HookAction> {
    match detect(lines, stop_hook_active) {
        DetectionOutcome::Allow | DetectionOutcome::NoMatch => None,
        DetectionOutcome::Block(cause) => {
            // Non-retryable errors should not be blocked
            if !cause.is_retryable() {
                return None;
            }

            Some(HookAction {
                wait_seconds: cause.wait_seconds(wait_seconds),
                output: HookOutput {
                    decision: "block".to_string(),
                    reason: cause.reason().to_string(),
                },
            })
        }
    }
}

// ============================================================================
// CLI Argument Parsing
// ============================================================================

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
                println!("cc-goto-work - Claude Code Stop Hook");
                println!();
                println!("Detects transient API errors and instructs Claude to continue working.");
                println!();
                println!("USAGE:");
                println!("    cc-goto-work [OPTIONS]");
                println!();
                println!("OPTIONS:");
                println!(
                    "    -w, --wait <SECONDS>    Wait time before continuing (default: {})",
                    DEFAULT_WAIT_SECONDS
                );
                println!("    -h, --help              Print help information");
                println!("    -V, --version           Print version information");
                println!();
                println!("DETECTED ERRORS:");
                println!("    - RESOURCE_EXHAUSTED (API quota/overload)");
                println!("    - Rate limits (HTTP 429)");
                println!("    - Server overload (HTTP 503/529)");
                println!("    - max_tokens (output truncation)");
                println!();
                println!("FATAL ERRORS (not retried):");
                println!("    - Context length exceeded");
                println!("    - Cost/spending limit reached");
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

    // Get transcript path
    let transcript_path = match &input.transcript_path {
        Some(path) => expand_path(path),
        None => return Ok(()), // No transcript, allow stop
    };

    let stop_hook_active = input.stop_hook_active.unwrap_or(false);

    // Read transcript tail and detect issues
    let lines = read_transcript_tail(&transcript_path)?;

    if let Some(action) = decide_action(&lines, stop_hook_active, wait_seconds) {
        // Wait before continuing (for rate limits, etc.)
        if action.wait_seconds > 0 {
            thread::sleep(Duration::from_secs(action.wait_seconds));
        }

        // Output the decision
        println!("{}", serde_json::to_string(&action.output)?);
    }

    // Exit 0 - if we printed JSON, it will be processed; otherwise stop is allowed
    Ok(())
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn line(raw: &str) -> TranscriptLine {
        TranscriptLine {
            raw: raw.to_string(),
            json: serde_json::from_str::<serde_json::Value>(raw).ok(),
        }
    }

    fn raw_line(raw: &str) -> TranscriptLine {
        TranscriptLine {
            raw: raw.to_string(),
            json: None,
        }
    }

    // ========== Fatal Error Tests ==========

    #[test]
    fn fatal_context_length_exceeded_allows_stop() {
        let lines = vec![line(
            r#"{"type":"error","error":{"type":"context_length_exceeded","message":"Context too long"}}"#,
        )];
        // Fatal errors return Block(cause) but is_retryable() returns false,
        // so decide_action returns None (allowing the stop)
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::ContextLengthExceeded)
        );
        assert!(decide_action(&lines, false, 30).is_none());
    }

    #[test]
    fn fatal_cost_limit_allows_stop() {
        let lines = vec![line(
            r#"{"type":"error","error":{"type":"error","message":"Cost limit exceeded for this session"}}"#,
        )];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::CostLimitReached)
        );
        assert!(decide_action(&lines, false, 30).is_none());
    }

    // ========== Stop Reason Boundary Tests ==========

    #[test]
    fn max_tokens_blocks_with_no_wait() {
        let lines = vec![line(
            r#"{"type":"assistant","message":{"stop_reason":"max_tokens"}}"#,
        )];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::MaxTokens)
        );
        let action = decide_action(&lines, false, 30).expect("should block");
        assert_eq!(action.wait_seconds, 0);
    }

    #[test]
    fn end_turn_allows_even_with_old_error() {
        let lines = vec![
            line(r#"{"type":"error","error":{"type":"RESOURCE_EXHAUSTED"}}"#),
            line(r#"{"type":"assistant","message":{"stop_reason":"end_turn"}}"#),
        ];
        assert_eq!(detect(&lines, false), DetectionOutcome::Allow);
        assert!(decide_action(&lines, false, 30).is_none());
    }

    // ========== Structured Error Tests ==========

    #[test]
    fn resource_exhausted_blocks_with_wait() {
        let lines = vec![line(
            r#"{"type":"error","error":{"type":"RESOURCE_EXHAUSTED","message":"Rate limit"}}"#,
        )];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::ResourceExhausted)
        );
        let action = decide_action(&lines, false, 30).expect("should block");
        assert_eq!(action.wait_seconds, 30);
    }

    #[test]
    fn rate_limit_error_blocks() {
        let lines = vec![line(
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"Too many requests"}}"#,
        )];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::RateLimited)
        );
    }

    // ========== HTTP Status Tests ==========

    #[test]
    fn http_429_blocks() {
        let lines = vec![line(
            r#"{"type":"error","status":429,"message":"Rate limited"}"#,
        )];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::RateLimited)
        );
    }

    #[test]
    fn http_503_blocks() {
        let lines = vec![line(
            r#"{"type":"error","error":{"status_code":503}}"#,
        )];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::Overloaded)
        );
    }

    // ========== Raw Fallback Tests ==========

    #[test]
    fn raw_fallback_detects_resource_exhausted() {
        let lines = vec![raw_line("Error: RESOURCE_EXHAUSTED - please try again")];
        assert_eq!(
            detect(&lines, false),
            DetectionOutcome::Block(StopCause::ResourceExhausted)
        );
    }

    #[test]
    fn raw_fallback_ignores_old_lines() {
        let mut lines = vec![raw_line("RESOURCE_EXHAUSTED")];
        // Add enough lines to push the error out of the fallback window
        for _ in 0..RAW_FALLBACK_LINES {
            lines.push(raw_line("normal line"));
        }
        // The error should be outside the window now
        assert_eq!(detect_raw_fallback(&lines, false), DetectionOutcome::NoMatch);
    }

    #[test]
    fn structured_error_ignores_old_lines() {
        let mut lines = vec![line(
            r#"{"type":"error","error":{"type":"RESOURCE_EXHAUSTED"}}"#,
        )];
        // Add enough normal lines to push the error out of the detection window
        for _ in 0..RECENT_ERROR_LINES {
            lines.push(line(r#"{"type":"user","message":{"content":"hello"}}"#));
        }
        // The error should be outside the window now
        assert_eq!(detect_structured_error(&lines, false), DetectionOutcome::NoMatch);
    }

    #[test]
    fn http_status_ignores_old_lines() {
        let mut lines = vec![line(
            r#"{"type":"error","status":429,"message":"Rate limited"}"#,
        )];
        // Add enough normal lines to push the error out of the detection window
        for _ in 0..RECENT_ERROR_LINES {
            lines.push(line(r#"{"type":"user","message":{"content":"hello"}}"#));
        }
        // The error should be outside the window now
        assert_eq!(detect_http_status(&lines, false), DetectionOutcome::NoMatch);
    }

    // ========== Integration Tests ==========

    #[test]
    fn retryable_error_always_blocks_even_when_stop_hook_active() {
        let lines = vec![line(
            r#"{"type":"error","error":{"type":"RESOURCE_EXHAUSTED"}}"#,
        )];
        // stop_hook_active should NOT prevent blocking for retryable errors
        assert!(decide_action(&lines, true, 30).is_some());
    }

    #[test]
    fn no_match_returns_none() {
        let lines = vec![line(r#"{"type":"user","message":{"content":"hello"}}"#)];
        assert!(decide_action(&lines, false, 30).is_none());
    }
}
