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
const INDEX_PAGE_URL: &str = "https://iw68lh.aliwork.com/o/fkxt_index_app";
const VISITOR_LOGIN_API: &str =
    "https://dingtalk.avaryholding.com:8443/dingplus/visitorConnector/visitorLogin";

fn build_init_script(phone: &str, code: &str) -> Result<String, String> {
    let phone_json = serde_json::to_string(phone)
        .map_err(|err| format!("failed to encode phone for init script: {err}"))?;
    let code_json = serde_json::to_string(code)
        .map_err(|err| format!("failed to encode code for init script: {err}"))?;
    let login_api_json = serde_json::to_string(VISITOR_LOGIN_API)
        .map_err(|err| format!("failed to encode login api for init script: {err}"))?;
    let index_page_json = serde_json::to_string(INDEX_PAGE_URL)
        .map_err(|err| format!("failed to encode index page for init script: {err}"))?;

    Ok(format!(
        r#"
        (function() {{
            if (window.__visitBatchAuthInitInstalled) {{
                return;
            }}
            window.__visitBatchAuthInitInstalled = true;

            const phone = {phone_json};
            const code = {code_json};
            const loginApi = {login_api_json};
            const indexPage = {index_page_json};

            const report = (kind, message) => {{
                const value = typeof message === "string" ? message : String(message || "");
                window.location.href = "app://" + kind + "/" + encodeURIComponent(value);
            }};

            const fail = (message) => report("login-error", message || "登录失败");

            const findTokenInText = value => {{
                if (typeof value !== "string") {{
                    return "";
                }}
                const match = value.match(/[A-Fa-f0-9]{{64,}}/);
                return match ? match[0] : "";
            }};

            const findTokenInObject = value => {{
                try {{
                    if (typeof value === "string") {{
                        return findTokenInText(value);
                    }}
                    if (!value || typeof value !== "object") {{
                        return "";
                    }}
                    if (Array.isArray(value)) {{
                        for (const item of value) {{
                            const token = findTokenInObject(item);
                            if (token) {{
                                return token;
                            }}
                        }}
                        return "";
                    }}
                    for (const [key, item] of Object.entries(value)) {{
                        const keyToken = findTokenInText(key);
                        if (keyToken) {{
                            return keyToken;
                        }}
                        const valueToken = findTokenInObject(item);
                        if (valueToken) {{
                            return valueToken;
                        }}
                    }}
                }} catch (_error) {{
                    return "";
                }}
                return "";
            }};

            const tryFindTokenInStorage = storage => {{
                if (!storage) {{
                    return "";
                }}
                try {{
                    const direct = storage.getItem("acToken") || storage.getItem("token") || "";
                    const directToken = findTokenInText(direct);
                    if (directToken) {{
                        return directToken;
                    }}

                    for (let i = 0; i < storage.length; i += 1) {{
                        const key = storage.key(i);
                        if (!key) {{
                            continue;
                        }}
                        const value = storage.getItem(key) || "";
                        const token = findTokenInText(value);
                        if (token) {{
                            return token;
                        }}
                    }}
                }} catch (_error) {{
                    return "";
                }}
                return "";
            }};

            const tryFindTokenInCookies = () => {{
                try {{
                    const cookieText = document.cookie || "";
                    for (const part of cookieText.split(";")) {{
                        const token = findTokenInText(part);
                        if (token) {{
                            return token;
                        }}
                    }}
                }} catch (_error) {{
                    return "";
                }}
                return "";
            }};

            const tryFindTokenInWindow = () => {{
                const candidates = [
                    window.acToken,
                    window.token,
                    window.__acToken,
                    window.__token
                ];
                for (const item of candidates) {{
                    const serialized =
                        typeof item === "string" ? item : JSON.stringify(item || "");
                    const token = findTokenInText(serialized);
                    if (token) {{
                        return token;
                    }}
                }}
                return "";
            }};

            const summarizeStorage = () => {{
                const parts = [];
                try {{
                    parts.push(
                        "localStorage=[" +
                            Array.from({{ length: localStorage.length }}, (_, i) => localStorage.key(i))
                                .filter(Boolean)
                                .join(",") +
                            "]"
                    );
                }} catch (_error) {{
                    parts.push("localStorage=[unavailable]");
                }}
                try {{
                    parts.push(
                        "sessionStorage=[" +
                            Array.from({{ length: sessionStorage.length }}, (_, i) => sessionStorage.key(i))
                                .filter(Boolean)
                                .join(",") +
                            "]"
                    );
                }} catch (_error) {{
                    parts.push("sessionStorage=[unavailable]");
                }}
                try {{
                    parts.push("cookie=" + ((document.cookie || "").slice(0, 200) || "<empty>"));
                }} catch (_error) {{
                    parts.push("cookie=[unavailable]");
                }}
                return parts.join(" ; ");
            }};

            const tryFindTokenEverywhereSync = () =>
                tryFindTokenInStorage(window.localStorage) ||
                tryFindTokenInStorage(window.sessionStorage) ||
                tryFindTokenInCookies() ||
                tryFindTokenInWindow();

            const tryFindTokenInIndexedDb = async () => {{
                if (!indexedDB || typeof indexedDB.databases !== "function") {{
                    return "";
                }}
                try {{
                    const dbs = await indexedDB.databases();
                    for (const dbInfo of dbs || []) {{
                        const name = dbInfo && dbInfo.name;
                        if (!name) {{
                            continue;
                        }}
                        const token = await new Promise(resolve => {{
                            let settled = false;
                            const finish = value => {{
                                if (!settled) {{
                                    settled = true;
                                    resolve(value || "");
                                }}
                            }};
                            const request = indexedDB.open(name);
                            request.onerror = () => finish("");
                            request.onsuccess = () => {{
                                try {{
                                    const db = request.result;
                                    const storeNames = Array.from(db.objectStoreNames || []);
                                    if (!storeNames.length) {{
                                        db.close();
                                        finish("");
                                        return;
                                    }}
                                    const tx = db.transaction(storeNames, "readonly");
                                    for (const storeName of storeNames) {{
                                        const store = tx.objectStore(storeName);
                                        const getAllReq = store.getAll();
                                        getAllReq.onsuccess = () => {{
                                            try {{
                                                const values = getAllReq.result || [];
                                                for (const value of values) {{
                                                    const token = findTokenInText(
                                                        typeof value === "string"
                                                            ? value
                                                            : JSON.stringify(value)
                                                    );
                                                    if (token) {{
                                                        db.close();
                                                        finish(token);
                                                        return;
                                                    }}
                                                }}
                                            }} catch (_error) {{}}
                                        }};
                                    }}
                                    tx.oncomplete = () => {{
                                        db.close();
                                        finish("");
                                    }};
                                    tx.onerror = () => {{
                                        db.close();
                                        finish("");
                                    }};
                                }} catch (_error) {{
                                    finish("");
                                }}
                            }};
                        }});
                        if (token) {{
                            return token;
                        }}
                    }}
                }} catch (_error) {{
                    return "";
                }}
                return "";
            }};

            const startTokenPolling = () => {{
                if (window.__visitBatchTokenPollingStarted) {{
                    return;
                }}
                window.__visitBatchTokenPollingStarted = true;
                report("login-stage", "开始轮询 acToken");

                let attempts = 0;
                const timer = setInterval(async function() {{
                    attempts += 1;
                    const token =
                        tryFindTokenEverywhereSync() ||
                        (attempts >= 6 ? await tryFindTokenInIndexedDb() : "");
                    if (token) {{
                        clearInterval(timer);
                        report("ac-token", token);
                        return;
                    }}

                    if (attempts >= 40) {{
                        clearInterval(timer);
                        let bodyText = "";
                        try {{
                            bodyText = ((document.body && document.body.innerText) || "")
                                .trim()
                                .slice(0, 200);
                        }} catch (_error) {{
                            bodyText = "";
                        }}
                        const storageSummary = summarizeStorage();
                        const reason = bodyText
                            ? "页面未写入 acToken: " + bodyText
                            : "页面未写入 acToken";
                        fail(reason + " ; " + storageSummary);
                    }}
                }}, 500);
            }};

            const runVisitorLogin = async () => {{
                if (window.__visitBatchVisitorLoginStarted) {{
                    return;
                }}
                window.__visitBatchVisitorLoginStarted = true;
                report("login-stage", "开始执行 visitorLogin");

                const controller =
                    typeof AbortController === "function" ? new AbortController() : null;
                const timeoutId = controller
                    ? setTimeout(() => controller.abort(), 15000)
                    : null;

                try {{
                    const response = await fetch(loginApi, {{
                        method: "POST",
                        credentials: "include",
                        headers: {{
                            "Accept": "application/json, text/json",
                            "Content-Type": "application/json"
                        }},
                        body: JSON.stringify({{
                            phoneNum: phone,
                            code: code
                        }}),
                        signal: controller ? controller.signal : undefined
                    }});
                    const text = await response.text();
                    let data = null;
                    try {{
                        data = JSON.parse(text);
                    }} catch (_error) {{
                        data = null;
                    }}

                    const responseToken = findTokenInText(text) || findTokenInObject(data);
                    if (responseToken) {{
                        report("ac-token", responseToken);
                        return;
                    }}

                    if (!response.ok) {{
                        fail("visitorLogin HTTP " + response.status + ": " + text);
                        return;
                    }}

                    if (!data || Number(data.code || 0) !== 200) {{
                        const message =
                            (data && (data.message || data.errorMsg)) ||
                            ("visitorLogin 返回异常: " + text);
                        fail(message);
                        return;
                    }}

                    report("login-stage", "visitorLogin 成功，进入首页");
                    window.location.href =
                        indexPage + "?account=" + encodeURIComponent(phone);
                }} catch (error) {{
                    if (error && typeof error === "object" && "name" in error && error.name === "AbortError") {{
                        fail("visitorLogin 请求超时");
                    }} else {{
                        fail(error instanceof Error ? error.message : String(error));
                    }}
                }} finally {{
                    if (timeoutId) {{
                        clearTimeout(timeoutId);
                    }}
                }}
            }};

            const handlePage = () => {{
                const href = window.location.href || "";
                if (href === window.__visitBatchLastHandledUrl) {{
                    return;
                }}
                window.__visitBatchLastHandledUrl = href;

                if (href.includes("fk_login_app")) {{
                    report("login-stage", "已进入登录页");
                    void runVisitorLogin();
                    return;
                }}

                if (href.includes("fkxt_index_app")) {{
                    report("login-stage", "已进入首页");
                    startTokenPolling();
                }}
            }};

            window.addEventListener("load", handlePage);
            window.addEventListener("pageshow", handlePage);
            setInterval(handlePage, 500);
            handlePage();
        }})();
        "#
    ))
}

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

pub fn start_login(app: &tauri::AppHandle, phone: String, code: String) -> Result<(), String> {
    // Close existing auth window if any
    if let Some(existing) = app.get_webview_window("auth-login") {
        let _ = existing.destroy();
    }

    let login_url = format!("{LOGIN_PAGE_URL}?account={phone}");
    let parsed_url = login_url
        .parse()
        .map_err(|e| format!("invalid login url: {e}"))?;

    let init_script = build_init_script(&phone, &code)?;

    let ctx = Arc::new(Mutex::new(LoginContext {
        phone: phone.clone(),
        app_handle: app.clone(),
        completed: false,
    }));

    let ctx_nav = ctx.clone();
    let ctx_load = ctx.clone();

    let _webview = WebviewWindowBuilder::new(app, "auth-login", WebviewUrl::External(parsed_url))
        .title("登录中...")
        .inner_size(400.0, 600.0)
        .visible(true)
        .initialization_script(&init_script)
        .on_navigation(move |nav_url| {
            if nav_url.scheme() != "app" {
                return true;
            }

            let host = nav_url.host_str().unwrap_or("");
            let path = nav_url.path().trim_start_matches('/');
            let decoded_path = percent_decode(path);

            if host == "login-stage" {
                if let Ok(guard) = ctx_nav.lock() {
                    let _ = guard.app_handle.emit(
                        "login-result",
                        json!({
                            "success": false,
                            "status": "progress",
                            "message": decoded_path
                        }),
                    );
                }
                return false;
            }

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

            let guard = match ctx_load.lock() {
                Ok(g) => g,
                Err(_) => return,
            };

            if guard.completed {
                return;
            }

            let page_url = payload.url().as_str().to_string();
            let app_handle = guard.app_handle.clone();
            drop(guard);

            let _ = webview.eval("window.dispatchEvent(new Event('pageshow'));");
            let _ = app_handle.emit(
                "login-result",
                json!({
                    "success": false,
                    "status": "progress",
                    "message": format!("页面加载完成: {page_url}")
                }),
            );
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
