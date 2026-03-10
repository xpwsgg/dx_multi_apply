use serde_json::Value;

use crate::http_common::{AUTH_API_BASE, MOBILE_USER_AGENT};

pub async fn send_code(phone: &str) -> Result<String, String> {
    let url = format!("{AUTH_API_BASE}/sendCode");
    let body = serde_json::json!({
        "phoneNum": phone,
        "areaCode": "86",
        "region": ""
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/json")
        .header("User-Agent", MOBILE_USER_AGENT)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("sendCode request failed: {e}"))?;

    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read sendCode response: {e}"))?;

    if !(200..=299).contains(&status) {
        return Err(format!("sendCode: status {status}, body: {text}"));
    }

    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("failed to parse sendCode json: {e}"))?;

    let resp_code = json.get("code").and_then(Value::as_i64).unwrap_or(0);
    if resp_code != 200 {
        let msg = json
            .get("message")
            .or_else(|| json.get("errorMsg"))
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(format!("sendCode failed: {msg}"));
    }

    let code = json
        .pointer("/data/code")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            other => other.to_string(),
        })
        .ok_or_else(|| format!("sendCode: missing data.code in response: {text}"))?;

    Ok(code.to_string())
}

pub async fn visitor_login(phone: &str, code: &str) -> Result<(), String> {
    let url = format!("{AUTH_API_BASE}/visitorLogin");
    let body = serde_json::json!({
        "phoneNum": phone,
        "code": code
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/json")
        .header("User-Agent", MOBILE_USER_AGENT)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("visitorLogin request failed: {e}"))?;

    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read visitorLogin response: {e}"))?;

    if !(200..=299).contains(&status) {
        return Err(format!("visitorLogin: status {status}, body: {text}"));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse visitorLogin json: {e}"))?;

    let resp_code = json.get("code").and_then(Value::as_i64).unwrap_or(0);
    if resp_code != 200 {
        let msg = json
            .get("message")
            .or_else(|| json.get("errorMsg"))
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(format!("visitorLogin failed: {msg}"));
    }

    Ok(())
}
