use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tauri::Manager;

const LOG_DIR_NAME: &str = "logs";
const LOG_FILE_NAME: &str = "app.log";
const MAX_LOG_FILE_BYTES: usize = 512 * 1024;
const MAX_LOG_STRING_CHARS: usize = 4 * 1024;

fn truncate_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let truncated: String = text.chars().take(max_chars).collect();
    format!("{truncated}...[truncated {} chars]", char_count - max_chars)
}

fn sanitize_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(truncate_text(text, MAX_LOG_STRING_CHARS))
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sanitize_value).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), sanitize_value(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn retain_recent_lines(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }

    let mut kept_lines = Vec::new();
    let mut total_bytes = 0usize;

    for line in content.lines().rev() {
        let line_bytes = line.len() + 1;
        if !kept_lines.is_empty() && total_bytes + line_bytes > max_bytes {
            break;
        }
        if kept_lines.is_empty() && line_bytes > max_bytes {
            kept_lines.push(truncate_text(line, max_bytes.saturating_sub(32)));
            break;
        }
        kept_lines.push(line.to_string());
        total_bytes += line_bytes;
    }

    kept_lines.reverse();
    let mut trimmed = kept_lines.join("\n");
    if !trimmed.is_empty() {
        trimmed.push('\n');
    }
    trimmed
}

fn compact_log_file(path: &PathBuf) -> Result<(), String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(format!(
                "failed to read log metadata {}: {err}",
                path.display()
            ))
        }
    };

    if metadata.len() as usize <= MAX_LOG_FILE_BYTES {
        return Ok(());
    }

    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read log file {}: {err}", path.display()))?;
    let trimmed = retain_recent_lines(&content, MAX_LOG_FILE_BYTES);
    fs::write(path, trimmed)
        .map_err(|err| format!("failed to compact log file {}: {err}", path.display()))?;
    Ok(())
}

fn log_file_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| format!("failed to resolve app data dir: {err}"))?;

    let log_dir = app_data_dir.join(LOG_DIR_NAME);
    fs::create_dir_all(&log_dir)
        .map_err(|err| format!("failed to create log dir {}: {err}", log_dir.display()))?;

    Ok(log_dir.join(LOG_FILE_NAME))
}

pub fn clear_log(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let path = log_file_path(app_handle)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("failed to remove log file {}: {err}", path.display()))?;
    }
    Ok(())
}

pub fn append_log(app_handle: &tauri::AppHandle, entry: &serde_json::Value) -> Result<(), String> {
    let path = log_file_path(app_handle)?;
    let sanitized = sanitize_value(entry);

    let mut line = serde_json::to_string(&sanitized)
        .map_err(|err| format!("failed to encode log entry: {err}"))?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("failed to open log file {}: {err}", path.display()))?;

    file.write_all(line.as_bytes())
        .map_err(|err| format!("failed to write log entry: {err}"))?;
    file.flush()
        .map_err(|err| format!("failed to flush log entry: {err}"))?;
    drop(file);
    compact_log_file(&path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{retain_recent_lines, truncate_text};

    #[test]
    fn should_truncate_long_text_with_suffix() {
        let text = "abcdef";
        let truncated = truncate_text(text, 3);
        assert!(truncated.starts_with("abc"));
        assert!(truncated.contains("[truncated 3 chars]"));
    }

    #[test]
    fn should_keep_recent_lines_within_byte_limit() {
        let content = "line-1\nline-2\nline-3\n";
        let trimmed = retain_recent_lines(content, 14);
        assert_eq!(trimmed, "line-2\nline-3\n");
    }
}
