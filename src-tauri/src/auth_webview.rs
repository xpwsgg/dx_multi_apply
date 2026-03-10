use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde_json::json;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::token_store::{self, TokenData};

struct LoginContext {
    phone: String,
    app_handle: tauri::AppHandle,
    completed: bool,
}

const LOGIN_PAGE_URL: &str = "https://iw68lh.aliwork.com/o/fk_login_app";

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::new();
    let src = input.as_bytes();
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'%' && i + 2 < src.len() {
            let hi = (src[i + 1] as char).to_digit(16);
            let lo = (src[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                bytes.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        bytes.push(src[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn start_login(app: &tauri::AppHandle, phone: String) -> Result<(), String> {
    // Close existing auth window if any
    if let Some(existing) = app.get_webview_window("auth-login") {
        let _ = existing.destroy();
    }

    let login_url = format!("{LOGIN_PAGE_URL}?account={phone}");
    let parsed_url = login_url
        .parse()
        .map_err(|e| format!("invalid login url: {e}"))?;

    let ctx = Arc::new(Mutex::new(LoginContext {
        phone: phone.clone(),
        app_handle: app.clone(),
        completed: false,
    }));

    let ctx_nav = ctx.clone();
    let ctx_load = ctx.clone();

    let _webview = WebviewWindowBuilder::new(
        app,
        "auth-login",
        WebviewUrl::External(parsed_url),
    )
    .title("登录中...")
    .inner_size(400.0, 600.0)
    .visible(false)
    .on_navigation(move |nav_url| {
        if nav_url.scheme() != "app" {
            return true;
        }

        let host = nav_url.host_str().unwrap_or("");
        let path = nav_url.path().trim_start_matches('/');
        let decoded_path = percent_decode(path);

        let mut guard = match ctx_nav.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        if guard.completed {
            return false;
        }
        guard.completed = true;

        let app_handle = guard.app_handle.clone();
        let phone = guard.phone.clone();
        drop(guard);

        match host {
            "ac-token" => {
                if decoded_path.is_empty() {
                    let _ = app_handle.emit(
                        "login-result",
                        json!({ "success": false, "error": "获取 acToken 为空" }),
                    );
                } else {
                    let token_data = TokenData {
                        ac_token: decoded_path,
                        phone: phone.clone(),
                        obtained_at: Utc::now().to_rfc3339(),
                    };
                    match token_store::save_token(&app_handle, &token_data) {
                        Ok(()) => {
                            let _ = app_handle.emit(
                                "login-result",
                                json!({
                                    "success": true,
                                    "phone": phone,
                                    "obtainedAt": token_data.obtained_at
                                }),
                            );
                        }
                        Err(err) => {
                            let _ = app_handle.emit(
                                "login-result",
                                json!({
                                    "success": false,
                                    "error": format!("保存 token 失败: {err}")
                                }),
                            );
                        }
                    }
                }
            }
            "login-error" => {
                let _ = app_handle.emit(
                    "login-result",
                    json!({
                        "success": false,
                        "error": format!("登录失败: {decoded_path}")
                    }),
                );
            }
            _ => {}
        }

        // Close webview asynchronously to avoid deadlock
        let handle = app_handle;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(win) = handle.get_webview_window("auth-login") {
                let _ = win.destroy();
            }
        });

        false
    })
    .on_page_load(move |webview, payload| {
        if payload.event() != tauri::webview::PageLoadEvent::Finished {
            return;
        }

        let page_url = payload.url().as_str();
        let guard = match ctx_load.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.completed {
            return;
        }

        let phone = guard.phone.clone();
        drop(guard);

        if page_url.contains("fk_login_app") {
            // visitorLogin already completed from Rust — navigate to index page for token extraction
            let js = format!(
                r#"window.location.href = 'https://iw68lh.aliwork.com/o/fkxt_index_app?account={phone}';"#
            );
            let _ = webview.eval(&js);
        } else if page_url.contains("fkxt_index_app") {
            // Index page loaded — wait 3s for JS to write acToken, then extract
            let js = r#"
                setTimeout(function() {
                    var token = localStorage.getItem('acToken') || '';
                    window.location.href = 'app://ac-token/' + encodeURIComponent(token);
                }, 3000);
            "#;
            let _ = webview.eval(js);
        }
    })
    .build()
    .map_err(|e| format!("failed to create auth webview: {e}"))?;

    // Timeout: close after 30s if not completed
    let ctx_timeout = ctx;
    let timeout_handle = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let mut guard = match ctx_timeout.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if !guard.completed {
            guard.completed = true;
            let _ = timeout_handle.emit(
                "login-result",
                json!({ "success": false, "error": "登录超时（30秒）" }),
            );
            drop(guard);

            if let Some(win) = timeout_handle.get_webview_window("auth-login") {
                let _ = win.destroy();
            }
        }
    });

    Ok(())
}
