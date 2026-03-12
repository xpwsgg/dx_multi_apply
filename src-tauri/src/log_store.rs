use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tauri::Manager;

const LOG_DIR_NAME: &str = "logs";
const LOG_FILE_NAME: &str = "app.log";

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

pub fn append_log(app_handle: &tauri::AppHandle, entry: &serde_json::Value) -> Result<(), String> {
    let path = log_file_path(app_handle)?;

    let mut line =
        serde_json::to_string(entry).map_err(|err| format!("failed to encode log entry: {err}"))?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("failed to open log file {}: {err}", path.display()))?;

    file.write_all(line.as_bytes())
        .map_err(|err| format!("failed to write log entry: {err}"))?;

    Ok(())
}
