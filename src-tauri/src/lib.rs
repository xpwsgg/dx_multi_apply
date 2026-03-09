mod app_state;
mod batch_runner;
mod date_mapping;
mod form_state_store;
mod history_store;
mod http_common;
mod log_store;
mod reception_client;
mod request_template;
mod submit_client;
mod visitor_client;

use chrono::{NaiveDate, Utc};
use rand::Rng;
use serde_json::json;
use tauri::Emitter;

use reception_client::ReceptionInfo;
use visitor_client::VisitorInfo;

#[cfg(test)]
mod app_state_tests;
#[cfg(test)]
mod batch_runner_tests;
#[cfg(test)]
mod request_template_tests;
#[cfg(test)]
mod submit_client_tests;

#[tauri::command]
async fn fetch_visitor_info(
    app_handle: tauri::AppHandle,
    account: String,
    id_card: String,
) -> Result<VisitorInfo, String> {
    let timestamp = Utc::now().to_rfc3339();
    match visitor_client::fetch_visitor_info(&account, &id_card).await {
        Ok((info, response_text)) => {
            let log_payload = json!({
                "result": "visitor_query",
                "reason": format!("身份证号 {id_card}"),
                "responseRaw": response_text
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(&app_handle, &json!({
                "timestamp": timestamp,
                "operation": "fetch_visitor_info",
                "request_summary": format!("account={account}, id_card={id_card}"),
                "status": 200,
                "response_body": response_text
            }));
            Ok(info)
        }
        Err(err) => {
            let log_payload = json!({
                "result": "visitor_query",
                "reason": format!("身份证号 {id_card} 查询失败: {err}"),
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(&app_handle, &json!({
                "timestamp": timestamp,
                "operation": "fetch_visitor_info",
                "request_summary": format!("account={account}, id_card={id_card}"),
                "status": 0,
                "response_body": err
            }));
            Err(err)
        }
    }
}

#[tauri::command]
async fn fetch_reception_info(
    app_handle: tauri::AppHandle,
    employee_id: String,
) -> Result<ReceptionInfo, String> {
    let timestamp = Utc::now().to_rfc3339();
    match reception_client::fetch_reception_info(&employee_id).await {
        Ok((info, response_text)) => {
            let log_payload = json!({
                "result": "reception_query",
                "reason": format!("员工号 {employee_id}"),
                "responseRaw": response_text
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(&app_handle, &json!({
                "timestamp": timestamp,
                "operation": "fetch_reception_info",
                "request_summary": format!("employee_id={employee_id}"),
                "status": 200,
                "response_body": response_text
            }));
            Ok(info)
        }
        Err(err) => {
            let log_payload = json!({
                "result": "reception_query",
                "reason": format!("员工号 {employee_id} 查询失败: {err}"),
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(&app_handle, &json!({
                "timestamp": timestamp,
                "operation": "fetch_reception_info",
                "request_summary": format!("employee_id={employee_id}"),
                "status": 0,
                "response_body": err
            }));
            Err(err)
        }
    }
}

#[tauri::command]
async fn start_batch_submit(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, app_state::AppState>,
    account: String,
    visitors: Vec<VisitorInfo>,
    receptions: Vec<ReceptionInfo>,
    dates: Vec<String>,
) -> Result<(), String> {
    app_state::validate_dates(&dates)?;
    app_state::reset_stop(&state);

    if visitors.is_empty() {
        return Err("至少需要一个访客".to_string());
    }

    if receptions.is_empty() {
        return Err("至少需要一个接待人".to_string());
    }

    // Save form state for next session
    let form_state = form_state_store::FormState {
        account: account.clone(),
        visitor_id_cards: visitors.iter().map(|v| v.id_card.clone()).collect(),
        reception_ids: receptions.iter().map(|r| r.employee_id.clone()).collect(),
    };
    let _ = form_state_store::save_form_state(&app_handle, &form_state);

    // Iterate through each reception and submit
    for reception in &receptions {
        let mut sorted_dates = dates.clone();
        sorted_dates.sort_unstable();
        let mut existing_keys = history_store::get_recent_history(&app_handle)?
            .into_iter()
            .map(|record| format!("{}-{}", record.date, record.reception_id))
            .collect::<std::collections::HashSet<_>>();

        for (index, date_text) in sorted_dates.iter().enumerate() {
            let date_text = date_text.clone();
            let key = format!("{}-{}", date_text, reception.employee_id);
            if app_state::is_stopped(&state) {
                let _ = app_handle.emit(
                    "batch-log",
                    json!({ "date": date_text, "result": "stopped", "reason": "manual stop" }),
                );
                return Err("批量提交已手动停止".to_string());
            }

            if existing_keys.contains(&key) {
                let _ = app_handle.emit(
                    "batch-log",
                    json!({
                        "date": date_text,
                        "result": "skipped",
                        "reason": format!("already exists in local history for reception {}", reception.employee_id)
                    }),
                );
                continue;
            }

            let date = NaiveDate::parse_from_str(&date_text, "%Y-%m-%d")
                .map_err(|err| format!("invalid date {date_text}: {err}"))?;

            match submit_client::submit_once(&account, &visitors, reception, date).await {
                Ok(submit_result) => {
                    existing_keys.insert(key.clone());
                    history_store::upsert_success_record(&app_handle, &date_text, &reception.employee_id)?;

                    let response_text = submit_result.response_text;
                    let _ = log_store::append_log(&app_handle, &json!({
                        "timestamp": Utc::now().to_rfc3339(),
                        "operation": "submit",
                        "request_summary": format!("date={}, reception={}", date_text, reception.employee_id),
                        "status": submit_result.status_code,
                        "response_body": response_text
                    }));

                    let has_pending_after_current = sorted_dates
                        .iter()
                        .skip(index + 1)
                        .any(|next_date| {
                            let next_key = format!("{}-{}", next_date, reception.employee_id);
                            !existing_keys.contains(&next_key)
                        });

                    let wait_seconds =
                        has_pending_after_current.then(|| rand::thread_rng().gen_range(30..=50));
                    let mut success_payload = json!({
                        "date": date_text,
                        "result": "success",
                        "reason": format!("接待人: {}", reception.name),
                        "responseRaw": response_text
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
                    let _ = log_store::append_log(&app_handle, &json!({
                        "timestamp": Utc::now().to_rfc3339(),
                        "operation": "submit",
                        "request_summary": format!("date={}, reception={}", date_text, reception.employee_id),
                        "status": 0,
                        "response_body": err.response_raw
                    }));
                    let _ = app_handle.emit(
                        "batch-log",
                        json!({
                            "date": date_text,
                            "result": "failed",
                            "reason": format!("接待人 {}: {}", reception.name, reason),
                            "responseRaw": err.response_raw
                        }),
                    );
                    return Err(format!("接待人 {}: {}", reception.name, reason));
                }
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

#[tauri::command]
fn get_recent_history(
    app_handle: tauri::AppHandle,
) -> Result<Vec<history_store::HistoryRecord>, String> {
    history_store::get_recent_history(&app_handle)
}

#[tauri::command]
fn get_existing_keys(
    app_handle: tauri::AppHandle,
    dates: Vec<String>,
    reception_id: String,
) -> Result<Vec<String>, String> {
    history_store::get_existing_keys(&app_handle, &dates, &reception_id)
}

#[tauri::command]
fn get_existing_dates(
    app_handle: tauri::AppHandle,
    dates: Vec<String>,
) -> Result<Vec<String>, String> {
    history_store::get_existing_dates(&app_handle, &dates)
}

#[tauri::command]
fn load_form_state(
    app_handle: tauri::AppHandle,
) -> Result<Option<form_state_store::FormState>, String> {
    form_state_store::load_form_state(&app_handle)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(app_state::AppState::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            fetch_visitor_info,
            fetch_reception_info,
            start_batch_submit,
            stop_batch_submit,
            get_recent_history,
            get_existing_keys,
            get_existing_dates,
            load_form_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
