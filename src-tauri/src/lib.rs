mod app_state;
mod batch_runner;
mod date_mapping;
mod request_template;
mod submit_client;

use chrono::NaiveDate;
use rand::Rng;
use serde_json::json;
use tauri::Emitter;

#[cfg(test)]
mod app_state_tests;
#[cfg(test)]
mod batch_runner_tests;
#[cfg(test)]
mod request_template_tests;
#[cfg(test)]
mod submit_client_tests;

#[tauri::command]
async fn start_batch_submit(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, app_state::AppState>,
    dates: Vec<String>,
) -> Result<(), String> {
    app_state::validate_dates(&dates)?;
    app_state::reset_stop(&state);

    let mut sorted_dates = dates;
    sorted_dates.sort_unstable();
    let total_dates = sorted_dates.len();

    for (index, date_text) in sorted_dates.into_iter().enumerate() {
        if app_state::is_stopped(&state) {
            let _ = app_handle.emit(
                "batch-log",
                json!({ "date": date_text, "result": "stopped", "reason": "manual stop" }),
            );
            return Err("batch stopped manually".to_string());
        }

        let date = NaiveDate::parse_from_str(&date_text, "%Y-%m-%d")
            .map_err(|err| format!("invalid date {date_text}: {err}"))?;

        match submit_client::submit_once(date).await {
            Ok(submit_result) => {
                let is_last = index + 1 == total_dates;
                let wait_seconds = if is_last {
                    None
                } else {
                    Some(rand::thread_rng().gen_range(60..=120))
                };
                let mut success_payload = json!({
                    "date": date_text,
                    "result": "success",
                    "responseRaw": submit_result.response_text
                });
                if let Some(wait_seconds) = wait_seconds {
                    success_payload["waitSeconds"] = json!(wait_seconds);
                }
                let _ = app_handle.emit("batch-log", success_payload);

                if let Some(wait_seconds) = wait_seconds {
                    if !app_state::is_stopped(&state) {
                        tokio::time::sleep(std::time::Duration::from_secs(wait_seconds)).await;
                    }
                }
            }
            Err(err) => {
                let reason = err.message.clone();
                let _ = app_handle.emit(
                    "batch-log",
                    json!({
                        "date": date_text,
                        "result": "failed",
                        "reason": reason,
                        "responseRaw": err.response_raw
                    }),
                );
                return Err(reason);
            }
        }
    }

    Ok(())
}

#[tauri::command]
fn stop_batch_submit(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    app_state::request_stop(&state);
    let _ = app_handle.emit(
        "batch-log",
        json!({ "result": "stopped", "reason": "manual stop requested" }),
    );
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(app_state::AppState::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            start_batch_submit,
            stop_batch_submit
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
