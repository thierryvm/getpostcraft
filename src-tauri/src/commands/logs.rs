use serde::Serialize;
use tauri::Manager;

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub level: String,
    pub timestamp: String,
    pub message: String,
}

/// Read the last `lines` lines from the app log file.
/// Returns parsed log entries (level + timestamp + message).
/// The log file is written by tauri-plugin-log in the app data dir.
#[tauri::command]
pub async fn get_app_logs(
    app: tauri::AppHandle,
    lines: Option<u32>,
) -> Result<Vec<LogEntry>, String> {
    use std::io::{BufRead, BufReader};

    let limit = lines.unwrap_or(200) as usize;

    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Cannot resolve log directory: {e}"))?;

    let log_file = log_dir.join("app.log");

    if !log_file.exists() {
        return Ok(vec![]);
    }

    let file = std::fs::File::open(&log_file).map_err(|e| format!("Cannot open log file: {e}"))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .collect();

    // Take the last `limit` lines
    let start = all_lines.len().saturating_sub(limit);
    let entries = all_lines[start..]
        .iter()
        .map(|line| parse_log_line(line))
        .collect();

    Ok(entries)
}

/// Return the path of the current log file (for "open in explorer" feature).
#[tauri::command]
pub fn get_log_file_path(app: tauri::AppHandle) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Cannot resolve log directory: {e}"))?;
    Ok(log_dir.join("app.log").to_string_lossy().to_string())
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Parse a tauri-plugin-log line.
/// Format: `YYYY-MM-DDTHH:MM:SS.sssZ [LEVEL] message`
/// Falls back gracefully if the format doesn't match.
fn parse_log_line(line: &str) -> LogEntry {
    // Try to extract level from bracket notation: [INFO], [WARN], [ERROR], [DEBUG]
    let level = if line.contains("[ERROR]") {
        "error"
    } else if line.contains("[WARN]") {
        "warn"
    } else if line.contains("[INFO]") {
        "info"
    } else if line.contains("[DEBUG]") {
        "debug"
    } else {
        "info"
    };

    // Try to extract timestamp (first token before a space)
    let (timestamp, message) = if let Some(space) = line.find(' ') {
        let ts = line[..space].trim_matches(|c| c == '[' || c == ']');
        let rest = line[space + 1..].trim();
        // Strip level bracket from message
        let msg = rest
            .trim_start_matches("[ERROR]")
            .trim_start_matches("[WARN]")
            .trim_start_matches("[INFO]")
            .trim_start_matches("[DEBUG]")
            .trim();
        (ts.to_string(), msg.to_string())
    } else {
        (String::new(), line.to_string())
    };

    LogEntry {
        level: level.to_string(),
        timestamp,
        message,
    }
}
