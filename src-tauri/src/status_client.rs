use chrono::{FixedOffset, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http_common::{auth_client, AUTH_API_BASE, MOBILE_USER_AGENT, ORIGIN};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitorStatusRecord {
    pub flow_num: String,
    pub visitor_name: String,
    pub visitor_phone: String,
    pub visit_company: String,
    pub visit_park: String,
    pub apply_type: String,
    pub r_person_name: String,
    pub r_person_phone: String,
    pub date_start: String,
    pub date_end: String,
    pub flow_status: String,
    pub create_time: String,
}

fn timestamp_to_date(ts_str: &str) -> String {
    if let Ok(ms) = ts_str.parse::<i64>() {
        let timezone = FixedOffset::east_opt(8 * 3600).expect("valid timezone offset");
        if let chrono::LocalResult::Single(dt) = timezone.timestamp_millis_opt(ms) {
            return dt.format("%Y-%m-%d").to_string();
        }
    }
    ts_str.to_string()
}

fn flow_status_text(code: &str) -> String {
    // 映射来源：宜搭页面自定义 JS（switch (+item.flowStatus)）
    match code {
        "1" => "审核中".to_string(),
        "3" => "审核拒绝".to_string(),
        "4" => "审核同意".to_string(),
        "5" => "审核通过，权限未生效".to_string(),
        "6" => "权限已生效".to_string(),
        "7" => "权限已失效".to_string(),
        other => other.to_string(),
    }
}

/// Check if a saved acToken is still valid by calling visitorStatus.
/// Returns Ok(true) if valid, Ok(false) if definitively expired/rejected,
/// Err if network or server error (token should be preserved).
pub async fn check_token_valid(phone: &str, ac_token: &str) -> Result<bool, String> {
    let url = format!("{AUTH_API_BASE}/visitorStatus");
    let body = serde_json::json!({
        "visitorIdNo": "",
        "regPerson": phone,
        "acToken": ac_token
    });

    let response = auth_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/json")
        .header("User-Agent", MOBILE_USER_AGENT)
        .header("Origin", ORIGIN)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络请求失败，无法验证登录状态: {e}"))?;

    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;

    // 服务端错误（5xx）视为临时故障，不应清除 token
    if status >= 500 {
        return Err(format!("服务端异常({status})，稍后重试"));
    }

    let json: Value = serde_json::from_str(&text).unwrap_or_default();
    let code = json.get("code").and_then(Value::as_i64);

    match code {
        Some(200) => Ok(true),
        // 401 表示 token 已失效
        Some(401) => Ok(false),
        // 其他 code（如 500 参数校验错误）说明认证已通过，token 有效
        Some(_) => Ok(true),
        None => Err(format!("服务端返回异常，无法判断登录状态: {text}")),
    }
}

pub async fn query_visitor_status(
    phone: &str,
    id_card: &str,
    ac_token: &str,
) -> Result<(Vec<VisitorStatusRecord>, String), String> {
    let url = format!("{AUTH_API_BASE}/visitorStatus");
    let body = serde_json::json!({
        "visitorIdNo": id_card,
        "regPerson": phone,
        "acToken": ac_token
    });

    let response = auth_client()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/json")
        .header("User-Agent", MOBILE_USER_AGENT)
        .header("Origin", ORIGIN)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("visitorStatus request failed: {e}"))?;

    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read visitorStatus response: {e}"))?;

    if !(200..=299).contains(&status) {
        return Err(format!("visitorStatus: status {status}, body: {text}"));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse visitorStatus json: {e}"))?;

    let resp_code = json.get("code").and_then(Value::as_i64).unwrap_or(0);
    if resp_code == 401 {
        return Err("登录已失效，请重新登录".to_string());
    }
    if resp_code != 200 {
        let msg = json
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("visitorStatus failed: {msg}"));
    }

    let data = json
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    fn str_field(item: &Value, key: &str) -> String {
        item.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    let records: Vec<VisitorStatusRecord> = data
        .iter()
        .filter_map(|item| {
            let flow_num = item.get("flowNum").and_then(Value::as_str)?.to_string();
            Some(VisitorStatusRecord {
                flow_num,
                visitor_name: str_field(item, "visitorName"),
                visitor_phone: str_field(item, "visitorPhone"),
                visit_company: str_field(item, "visitCompanyName"),
                visit_park: str_field(item, "gardenName"),
                apply_type: str_field(item, "visitorType"),
                r_person_name: str_field(item, "rPersonName"),
                r_person_phone: str_field(item, "rPersonPhone"),
                date_start: timestamp_to_date(&str_field(item, "dateStart")),
                date_end: timestamp_to_date(&str_field(item, "dateEnd")),
                flow_status: flow_status_text(&str_field(item, "flowStatus")),
                create_time: str_field(item, "createTime"),
            })
        })
        .collect();

    Ok((records, text))
}

#[cfg(test)]
mod tests {
    use super::timestamp_to_date;

    #[test]
    fn should_convert_status_timestamp_using_utc_plus_8() {
        assert_eq!(timestamp_to_date("1775145600000"), "2026-04-03");
        assert_eq!(timestamp_to_date("1775059200000"), "2026-04-02");
    }

    #[test]
    fn should_keep_original_text_when_timestamp_is_invalid() {
        assert_eq!(timestamp_to_date("not-a-timestamp"), "not-a-timestamp");
    }
}
