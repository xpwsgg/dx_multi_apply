mod app_state;
mod auth_client;

#[cfg(test)]
mod batch_runner;
mod date_mapping;
mod form_state_store;
mod http_common;
mod log_store;
mod reception_client;
mod record_store;
mod request_template;
mod status_client;
mod submit_client;
mod token_store;
mod visitor_client;

use chrono::{NaiveDate, Utc};
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use tauri::{Emitter, Manager};

use reception_client::ReceptionInfo;
use serde::Serialize;
use status_client::VisitorStatusRecord;
use visitor_client::VisitorInfo;

#[cfg(test)]
mod app_state_tests;
#[cfg(test)]
mod batch_runner_tests;
#[cfg(test)]
mod request_template_tests;
#[cfg(test)]
mod submit_client_tests;

fn serialize_visitor_ids(visitor_id_cards: Vec<String>) -> String {
    let mut ids: Vec<String> = visitor_id_cards
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    ids.sort_unstable();
    ids.join(",")
}

fn has_pending_tasks_after_current(
    tasks: &[SubmissionTask],
    current_index: usize,
    force_resubmit: bool,
    existing_keys_by_reception: &std::collections::HashMap<
        String,
        std::collections::HashSet<String>,
    >,
) -> bool {
    tasks.iter().skip(current_index + 1).any(|next_task| {
        force_resubmit || {
            let next_key = format!("{}-{}", next_task.date, next_task.reception_id);
            !existing_keys_by_reception
                .get(&next_task.reception_id)
                .is_some_and(|existing_keys| existing_keys.contains(&next_key))
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmissionTask {
    date: String,
    reception_id: String,
}

#[tauri::command]
async fn fetch_visitor_info(
    app_handle: tauri::AppHandle,
    visitor_phone: Option<String>,
    account: String,
    id_card: String,
) -> Result<VisitorInfo, String> {
    let phone = match visitor_phone.as_deref() {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => account.trim().to_string(),
    };

    if phone.is_empty() {
        return Err("请提供访客手机号或申请人手机号".to_string());
    }

    let timestamp = Utc::now().to_rfc3339();
    match visitor_client::fetch_visitor_info(&phone, &id_card).await {
        Ok((info, response_text)) => {
            let log_payload = json!({
                "result": "visitor_query",
                "reason": format!("身份证号 {id_card}"),
                "responseRaw": response_text
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "fetch_visitor_info",
                    "request_summary": format!("phone={phone}, id_card={id_card}"),
                    "response_body": response_text
                }),
            );
            Ok(info)
        }
        Err(err) => {
            let log_payload = json!({
                "result": "visitor_query",
                "reason": format!("身份证号 {id_card} 查询失败: {err}"),
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "fetch_visitor_info",
                    "request_summary": format!("phone={phone}, id_card={id_card}"),
                    "status": 0,
                    "response_body": err
                }),
            );
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
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "fetch_reception_info",
                    "request_summary": format!("employee_id={employee_id}"),
                    "status": 200,
                    "response_body": response_text
                }),
            );
            Ok(info)
        }
        Err(err) => {
            let log_payload = json!({
                "result": "reception_query",
                "reason": format!("员工号 {employee_id} 查询失败: {err}"),
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "fetch_reception_info",
                    "request_summary": format!("employee_id={employee_id}"),
                    "status": 0,
                    "response_body": err
                }),
            );
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
    tasks: Vec<SubmissionTask>,
    force_resubmit: bool,
) -> Result<(), String> {
    if visitors.is_empty() {
        return Err("至少需要一个访客".to_string());
    }

    if receptions.is_empty() {
        return Err("至少需要一个接待人".to_string());
    }

    app_state::validate_non_empty_task_list(&tasks)?;
    app_state::try_start(&state)?;
    let result = async {
        let form_state = form_state_store::FormState {
            account: account.clone(),
            visitor_id_cards: visitors.iter().map(|v| v.id_card.clone()).collect(),
            visitor_phones: visitors.iter().map(|v| v.phone.clone()).collect(),
            reception_ids: receptions.iter().map(|r| r.employee_id.clone()).collect(),
        };
        let _ = form_state_store::save_form_state(&app_handle, &form_state);

        let mut fail_count: u32 = 0;
        let visitor_ids = serialize_visitor_ids(
            visitors
                .iter()
                .map(|v| v.id_card.clone())
                .collect::<Vec<String>>(),
        );

        let reception_map = receptions
            .iter()
            .cloned()
            .map(|reception| (reception.employee_id.clone(), reception))
            .collect::<std::collections::HashMap<_, _>>();

        let mut dates_by_reception = std::collections::HashMap::<String, Vec<String>>::new();
        for task in &tasks {
            dates_by_reception
                .entry(task.reception_id.clone())
                .or_default()
                .push(task.date.clone());
        }

        let mut existing_keys_by_reception =
            std::collections::HashMap::<String, std::collections::HashSet<String>>::new();
        for (reception_id, reception_dates) in &mut dates_by_reception {
            reception_dates.sort_unstable();
            reception_dates.dedup();
            let existing_keys = record_store::get_existing_keys(
                &app_handle,
                reception_dates,
                reception_id,
                &visitor_ids,
            )?
            .into_iter()
            .map(|date| format!("{}-{}", date, reception_id))
            .collect::<std::collections::HashSet<_>>();
            existing_keys_by_reception.insert(reception_id.clone(), existing_keys);
        }

        for (index, task) in tasks.iter().enumerate() {
            let date_text = task.date.clone();
            let reception = reception_map
                .get(&task.reception_id)
                .ok_or_else(|| format!("未找到接待人 {}", task.reception_id))?;
            let key = format!("{}-{}", date_text, reception.employee_id);

            if app_state::is_stopped(&state) {
                let _ = app_handle.emit(
                    "batch-log",
                    json!({
                        "date": date_text,
                        "receptionId": reception.employee_id,
                        "result": "stopped",
                        "reason": "manual stop"
                    }),
                );
                return Err("批量提交已手动停止".to_string());
            }

            if !force_resubmit
                && existing_keys_by_reception
                    .get(&task.reception_id)
                    .is_some_and(|existing_keys| existing_keys.contains(&key))
            {
                let _ = app_handle.emit(
                    "batch-log",
                    json!({
                        "date": date_text,
                        "receptionId": reception.employee_id,
                        "result": "skipped",
                        "reason": format!("already exists in local history for reception {}", reception.employee_id)
                    }),
                );
                continue;
            }

            let date = NaiveDate::parse_from_str(&date_text, "%Y-%m-%d")
                .map_err(|err| format!("invalid date {date_text}: {err}"))?;

            let has_pending_after_current = has_pending_tasks_after_current(
                &tasks,
                index,
                force_resubmit,
                &existing_keys_by_reception,
            );

            match submit_client::submit_once(&account, &visitors, reception, date).await {
                Ok(submit_result) => {
                    if let Some(existing_keys) =
                        existing_keys_by_reception.get_mut(&task.reception_id)
                    {
                        existing_keys.insert(key.clone());
                    }
                    record_store::upsert_record(
                        &app_handle,
                        &date_text,
                        &reception.employee_id,
                        &visitor_ids,
                    )?;

                    let response_text = submit_result.response_text;
                    let _ = log_store::append_log(
                        &app_handle,
                        &json!({
                            "timestamp": Utc::now().to_rfc3339(),
                            "operation": "submit",
                            "request_summary": format!("date={}, reception={}", date_text, reception.employee_id),
                            "status": submit_result.status_code,
                            "response_body": response_text
                        }),
                    );

                    let wait_seconds =
                        has_pending_after_current.then(|| rand::thread_rng().gen_range(30..=50));
                    let mut success_payload = json!({
                        "date": date_text,
                        "receptionId": reception.employee_id,
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
                    fail_count += 1;
                    let reason = err.message.clone();
                    let _ = log_store::append_log(
                        &app_handle,
                        &json!({
                            "timestamp": Utc::now().to_rfc3339(),
                            "operation": "submit",
                            "request_summary": format!("date={}, reception={}", date_text, reception.employee_id),
                            "status": 0,
                            "response_body": err.response_raw
                        }),
                    );

                    let wait_seconds =
                        has_pending_after_current.then(|| rand::thread_rng().gen_range(30..=50));
                    let mut failed_payload = json!({
                        "date": date_text,
                        "receptionId": reception.employee_id,
                        "result": "failed",
                        "reason": format!("接待人 {}: {}", reception.name, reason),
                        "responseRaw": err.response_raw
                    });
                    if let Some(wait_seconds) = wait_seconds {
                        failed_payload["waitSeconds"] = json!(wait_seconds);
                    }
                    let _ = app_handle.emit("batch-log", failed_payload);

                    if let Some(wait_seconds) = wait_seconds {
                        if !app_state::is_stopped(&state) {
                            tokio::time::sleep(std::time::Duration::from_secs(wait_seconds)).await;
                        }
                    }
                }
            }
        }

        if fail_count > 0 {
            return Err(format!("批量提交完成，其中 {fail_count} 条失败"));
        }
        Ok(())
    }
    .await;

    app_state::finish(&state);
    result
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
fn get_existing_keys(
    app_handle: tauri::AppHandle,
    dates: Vec<String>,
    reception_id: String,
    visitor_id_cards: Vec<String>,
) -> Result<Vec<String>, String> {
    let visitor_ids = serialize_visitor_ids(visitor_id_cards);
    if visitor_ids.is_empty() {
        return Ok(Vec::new());
    }
    record_store::get_existing_keys(&app_handle, &dates, &reception_id, &visitor_ids)
}

#[tauri::command]
fn get_existing_dates(
    app_handle: tauri::AppHandle,
    dates: Vec<String>,
    visitor_id_cards: Vec<String>,
) -> Result<Vec<String>, String> {
    let visitor_ids = serialize_visitor_ids(visitor_id_cards);
    if visitor_ids.is_empty() {
        return Ok(Vec::new());
    }
    record_store::get_existing_dates(&app_handle, &dates, &visitor_ids)
}

#[tauri::command]
fn load_form_state(
    app_handle: tauri::AppHandle,
) -> Result<Option<form_state_store::FormState>, String> {
    form_state_store::load_form_state(&app_handle)
}

#[tauri::command]
fn save_form_state(
    app_handle: tauri::AppHandle,
    account: String,
    visitor_id_cards: Vec<String>,
    visitor_phones: Option<Vec<String>>,
    reception_ids: Vec<String>,
) -> Result<(), String> {
    let state = form_state_store::FormState {
        account,
        visitor_id_cards,
        visitor_phones: visitor_phones.unwrap_or_default(),
        reception_ids,
    };
    form_state_store::save_form_state(&app_handle, &state)
}

#[tauri::command]
fn get_factory_info() -> std::collections::HashMap<String, String> {
    let mut info = std::collections::HashMap::new();
    info.insert("company".to_string(), http_common::COMPANY.to_string());
    info.insert("part".to_string(), http_common::PART.to_string());
    info.insert("applyType".to_string(), http_common::APPLY_TYPE.to_string());
    info
}

#[tauri::command]
async fn start_login(app_handle: tauri::AppHandle, account: String) -> Result<(), String> {
    let phone = account.trim().to_string();
    if phone.is_empty() {
        return Err("手机号不能为空".to_string());
    }

    let _ = app_handle.emit(
        "login-result",
        json!({ "success": false, "status": "sending_code" }),
    );

    let timestamp = Utc::now().to_rfc3339();
    let code = match auth_client::send_code(&phone).await {
        Ok(c) => {
            let _ = app_handle.emit(
                "batch-log",
                json!({
                    "result": "login_send_code",
                    "reason": format!("手机号 {phone} 验证码获取成功")
                }),
            );
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "send_code",
                    "request_summary": format!("phone={phone}"),
                    "status": 200
                }),
            );
            c
        }
        Err(e) => {
            let _ = app_handle.emit(
                "batch-log",
                json!({
                    "result": "login_send_code_failed",
                    "reason": format!("手机号 {phone} | {e}")
                }),
            );
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "send_code",
                    "request_summary": format!("phone={phone}"),
                    "error": e
                }),
            );
            return Err(e);
        }
    };

    let _ = app_handle.emit(
        "login-result",
        json!({ "success": false, "status": "progress", "message": "验证码已获取，正在登录..." }),
    );

    let timestamp = Utc::now().to_rfc3339();
    let ac_token = match auth_client::visitor_login(&phone, &code).await {
        Ok(token) => {
            let _ = app_handle.emit(
                "batch-log",
                json!({
                    "result": "login_visitor_login",
                    "reason": format!("手机号 {phone} 登录成功")
                }),
            );
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "visitor_login",
                    "request_summary": format!("phone={phone}"),
                    "status": 200
                }),
            );
            token
        }
        Err(e) => {
            let _ = app_handle.emit(
                "batch-log",
                json!({
                    "result": "login_visitor_login_failed",
                    "reason": format!("手机号 {phone} | {e}")
                }),
            );
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "visitor_login",
                    "request_summary": format!("phone={phone}"),
                    "error": e
                }),
            );
            return Err(e);
        }
    };

    let token_data = token_store::TokenData {
        ac_token,
        phone: phone.clone(),
        obtained_at: Utc::now().to_rfc3339(),
    };
    token_store::save_token(&app_handle, &token_data)?;

    let _ = app_handle.emit(
        "login-result",
        json!({
            "success": true,
            "phone": phone,
            "obtainedAt": token_data.obtained_at
        }),
    );

    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenStatus {
    phone: String,
    obtained_at: String,
}

#[tauri::command]
fn import_token(
    app_handle: tauri::AppHandle,
    account: String,
    ac_token: String,
) -> Result<TokenStatus, String> {
    let phone = account.trim().to_string();
    if phone.is_empty() {
        return Err("手机号不能为空".to_string());
    }

    let token = ac_token.trim().to_string();
    if token.len() < 64 || !token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("acToken 格式无效，请粘贴浏览器里完整的 64 位十六进制值".to_string());
    }

    let token_data = token_store::TokenData {
        ac_token: token,
        phone: phone.clone(),
        obtained_at: Utc::now().to_rfc3339(),
    };
    token_store::save_token(&app_handle, &token_data)?;

    Ok(TokenStatus {
        phone,
        obtained_at: token_data.obtained_at,
    })
}

#[tauri::command]
fn get_token_status(app_handle: tauri::AppHandle) -> Result<Option<TokenStatus>, String> {
    let data = token_store::load_token(&app_handle)?;
    Ok(data.map(|d| TokenStatus {
        phone: d.phone,
        obtained_at: d.obtained_at,
    }))
}

#[tauri::command]
async fn check_token(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let token_data = match token_store::load_token(&app_handle)? {
        Some(d) => d,
        None => return Ok(false),
    };
    let timestamp = Utc::now().to_rfc3339();
    let phone = &token_data.phone;
    match status_client::check_token_valid(phone, &token_data.ac_token).await {
        Ok(valid) => {
            let _ = app_handle.emit(
                "batch-log",
                json!({
                    "result": "check_token",
                    "reason": if valid {
                        format!("手机号 {phone} 登录状态有效")
                    } else {
                        format!("手机号 {phone} 登录已失效")
                    }
                }),
            );
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "check_token",
                    "request_summary": format!("phone={phone}"),
                    "status": 200,
                    "valid": valid
                }),
            );
            Ok(valid)
        }
        Err(e) => {
            let _ = app_handle.emit(
                "batch-log",
                json!({
                    "result": "check_token_failed",
                    "reason": format!("手机号 {phone} | {e}")
                }),
            );
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "check_token",
                    "request_summary": format!("phone={phone}"),
                    "error": e
                }),
            );
            Err(e)
        }
    }
}

#[tauri::command]
async fn query_visitor_status(
    app_handle: tauri::AppHandle,
    id_card: String,
) -> Result<Vec<VisitorStatusRecord>, String> {
    let token_data = token_store::load_token(&app_handle)?
        .ok_or_else(|| "未登录，请先登录获取 token".to_string())?;

    let phone = token_data.phone.clone();

    let timestamp = Utc::now().to_rfc3339();
    match status_client::query_visitor_status(&phone, &id_card, &token_data.ac_token)
        .await
    {
        Ok((records, response_text)) => {
            let log_payload = json!({
                "result": "status_query",
                "reason": format!("身份证号 {} | 共 {} 条记录", id_card, records.len()),
                "responseRaw": response_text
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "query_visitor_status",
                    "request_summary": format!("id_card={id_card}"),
                    "status": 200,
                    "record_count": records.len(),
                    "response_body": response_text
                }),
            );
            Ok(records)
        }
        Err(e) => {
            let log_payload = json!({
                "result": "status_query_failed",
                "reason": format!("身份证号 {} | {}", id_card, e),
            });
            let _ = app_handle.emit("batch-log", &log_payload);
            let _ = log_store::append_log(
                &app_handle,
                &json!({
                    "timestamp": timestamp,
                    "operation": "query_visitor_status",
                    "request_summary": format!("id_card={id_card}"),
                    "error": e
                }),
            );
            Err(e)
        }
    }
}

#[tauri::command]
fn clear_log(app_handle: tauri::AppHandle) -> Result<(), String> {
    log_store::clear_log(&app_handle)
}

#[tauri::command]
fn clear_token(app_handle: tauri::AppHandle) -> Result<(), String> {
    token_store::clear_token(&app_handle)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(app_state::AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            log_store::clear_log(&app.handle())?;
            record_store::init_db(&app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fetch_visitor_info,
            fetch_reception_info,
            start_batch_submit,
            stop_batch_submit,
            get_existing_keys,
            get_existing_dates,
            load_form_state,
            save_form_state,
            get_factory_info,
            start_login,
            import_token,
            get_token_status,
            check_token,
            query_visitor_status,
            clear_log,
            clear_token
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod start_batch_submit_tests {
    use super::{has_pending_tasks_after_current, SubmissionTask};

    #[test]
    fn should_not_wait_after_last_task_when_force_resubmit_is_enabled() {
        let tasks = vec![
            SubmissionTask {
                date: "2026-04-01".to_string(),
                reception_id: "R1".to_string(),
            },
            SubmissionTask {
                date: "2026-04-02".to_string(),
                reception_id: "R1".to_string(),
            },
        ];

        let existing_keys_by_reception = std::collections::HashMap::from([(
            "R1".to_string(),
            std::collections::HashSet::from([
                "2026-04-01-R1".to_string(),
                "2026-04-02-R1".to_string(),
            ]),
        )]);

        assert!(has_pending_tasks_after_current(
            &tasks,
            0,
            true,
            &existing_keys_by_reception,
        ));
        assert!(!has_pending_tasks_after_current(
            &tasks,
            1,
            true,
            &existing_keys_by_reception,
        ));
    }

    #[test]
    fn should_skip_wait_when_remaining_tasks_are_all_existing_and_not_force_resubmit() {
        let tasks = vec![
            SubmissionTask {
                date: "2026-04-01".to_string(),
                reception_id: "R1".to_string(),
            },
            SubmissionTask {
                date: "2026-04-02".to_string(),
                reception_id: "R1".to_string(),
            },
        ];

        let existing_keys_by_reception = std::collections::HashMap::from([(
            "R1".to_string(),
            std::collections::HashSet::from(["2026-04-02-R1".to_string()]),
        )]);

        assert!(!has_pending_tasks_after_current(
            &tasks,
            0,
            false,
            &existing_keys_by_reception,
        ));
    }
}
