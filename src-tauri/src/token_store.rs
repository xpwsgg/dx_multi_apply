use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

const TOKEN_FILE_NAME: &str = "ac_token.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenData {
    pub ac_token: String,
    pub phone: String,
    pub obtained_at: String,
}

fn token_file_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| format!("failed to resolve app data dir: {err}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|err| {
        format!(
            "failed to create app data dir {}: {err}",
            app_data_dir.display()
        )
    })?;

    Ok(app_data_dir.join(TOKEN_FILE_NAME))
}

pub fn save_token(app_handle: &tauri::AppHandle, data: &TokenData) -> Result<(), String> {
    let path = token_file_path(app_handle)?;

    let content = serde_json::to_string_pretty(data)
        .map_err(|err| format!("failed to encode token data: {err}"))?;

    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|err| {
        format!(
            "failed to write temp token file {}: {err}",
            temp_path.display()
        )
    })?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("failed to replace token file {}: {err}", path.display()))?;
    }

    fs::rename(&temp_path, &path).map_err(|err| {
        format!(
            "failed to rename temp token file {} to {}: {err}",
            temp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

pub fn load_token(app_handle: &tauri::AppHandle) -> Result<Option<TokenData>, String> {
    let path = token_file_path(app_handle)?;

    if !path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read token file {}: {err}", path.display()))?;

    if text.trim().is_empty() {
        return Ok(None);
    }

    let data: TokenData = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse token file {}: {err}", path.display()))?;

    Ok(Some(data))
}

pub fn clear_token(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let path = token_file_path(app_handle)?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("failed to remove token file {}: {err}", path.display()))?;
    }

    Ok(())
}
