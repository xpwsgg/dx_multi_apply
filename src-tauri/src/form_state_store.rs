use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

const FORM_STATE_FILE_NAME: &str = "form_state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormState {
    pub account: String,
    pub visitor_id_cards: Vec<String>,
    #[serde(default)]
    pub visitor_phones: Vec<String>,
    pub reception_ids: Vec<String>,
}

fn form_state_file_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
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

    Ok(app_data_dir.join(FORM_STATE_FILE_NAME))
}

pub fn save_form_state(app_handle: &tauri::AppHandle, state: &FormState) -> Result<(), String> {
    let path = form_state_file_path(app_handle)?;

    let content = serde_json::to_string_pretty(state)
        .map_err(|err| format!("failed to encode form state: {err}"))?;

    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|err| {
        format!(
            "failed to write temp form state file {}: {err}",
            temp_path.display()
        )
    })?;

    if path.exists() {
        fs::remove_file(&path).map_err(|err| {
            format!(
                "failed to replace form state file {}: {err}",
                path.display()
            )
        })?;
    }

    fs::rename(&temp_path, &path).map_err(|err| {
        format!(
            "failed to rename temp form state file {} to {}: {err}",
            temp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

pub fn load_form_state(app_handle: &tauri::AppHandle) -> Result<Option<FormState>, String> {
    let path = form_state_file_path(app_handle)?;

    if !path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read form state file {}: {err}", path.display()))?;

    if text.trim().is_empty() {
        return Ok(None);
    }

    let state: FormState = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse form state file {}: {err}", path.display()))?;

    Ok(Some(state))
}
