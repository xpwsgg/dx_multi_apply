use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http_common::{
    build_referer, ACCEPT, ACCEPT_LANGUAGE, APP_TYPE, BX_V, COOKIE, CSRF_TOKEN, FORM_UUID, ORIGIN,
    USER_AGENT,
};

const FETCH_URL: &str = "https://iw68lh.aliwork.com/o/HW9663A19D6M1QDL6D7GNAO1L2ZC26DXQHOXL7";

const FETCH_DATA_TEMPLATE: &str = include_str!("visitor_fetch_data.json");
const BINDING_FORMULAS: &str = include_str!("binding_formulas.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitorInfo {
    pub id_card: String,
    pub name: String,
    pub phone: String,
    pub photo: Value,
    pub id_photo: Value,
}

fn build_fetch_data(account: &str, id_card: &str) -> Result<Value, String> {
    let mut data: Value = serde_json::from_str(FETCH_DATA_TEMPLATE).map_err(|e| e.to_string())?;
    let fields = data
        .as_array_mut()
        .ok_or_else(|| "获取数据模板不是数组".to_string())?;

    for field in fields.iter_mut() {
        let field_id = field
            .get("fieldId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        if field_id == "textField_ly2ugh3m" {
            field["fieldData"]["value"] = Value::String(account.to_string());
        }

        if field_id == "tableField_lxv44os5" {
            if let Some(rows) = field
                .get_mut("fieldData")
                .and_then(|fd| fd.get_mut("value"))
                .and_then(Value::as_array_mut)
            {
                if let Some(row) = rows.first_mut().and_then(Value::as_array_mut) {
                    for cell in row.iter_mut() {
                        let cell_id = cell
                            .get("fieldId")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if cell_id == "textField_lxv44ory" {
                            cell["fieldData"]["value"] = Value::String(id_card.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(data)
}

fn parse_json_string_array(arr: &Value) -> Result<Value, String> {
    let items = arr
        .as_array()
        .ok_or_else(|| "媒体字段需要数组格式".to_string())?;
    let mut result = Vec::new();
    for item in items {
        if let Some(s) = item.as_str() {
            let obj: Value =
                serde_json::from_str(s).map_err(|e| format!("解析媒体JSON失败: {e}"))?;
            result.push(obj);
        } else {
            result.push(item.clone());
        }
    }
    Ok(Value::Array(result))
}

fn find_row_field_str(row: &[Value], field_id: &str) -> Result<String, String> {
    for cell in row {
        if cell.get("fieldId").and_then(Value::as_str) == Some(field_id) {
            return cell
                .pointer("/fieldData/value")
                .and_then(Value::as_str)
                .map(String::from)
                .ok_or_else(|| format!("字段 {field_id} 缺少字符串值"));
        }
    }
    Err(format!("行中未找到字段 {field_id}"))
}

fn find_row_field_value(row: &[Value], field_id: &str) -> Result<Value, String> {
    for cell in row {
        if cell.get("fieldId").and_then(Value::as_str) == Some(field_id) {
            return cell
                .pointer("/fieldData/value")
                .cloned()
                .ok_or_else(|| format!("missing value for {field_id}"));
        }
    }
    Err(format!("行中未找到字段 {field_id}"))
}

fn extract_visitor_from_response(id_card: &str, body: &Value) -> Result<VisitorInfo, String> {
    let success = body
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !success {
        return Err(format!(
            "visitor API returned success=false: {}",
            serde_json::to_string(body).unwrap_or_default()
        ));
    }

    let data = body
        .pointer("/content/data")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing content.data in response".to_string())?;

    let table = data
        .iter()
        .find(|f| f.get("fieldId").and_then(Value::as_str) == Some("tableField_lxv44os5"))
        .ok_or_else(|| "missing tableField_lxv44os5 in response".to_string())?;

    let rows = table
        .pointer("/fieldData/value")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing table fieldData.value".to_string())?;

    let row = rows
        .first()
        .and_then(Value::as_array)
        .ok_or_else(|| "missing first table row".to_string())?;

    let name = find_row_field_str(row, "textField_lxv44orw")?;
    let phone = find_row_field_str(row, "textField_lxv44orz")?;
    let photo_raw = find_row_field_value(row, "imageField_ly9i5k5q")?;
    let id_photo_raw = find_row_field_value(row, "attachmentField_lxv44osj")?;

    let photo = parse_json_string_array(&photo_raw)?;
    let id_photo = parse_json_string_array(&id_photo_raw)?;

    if name.is_empty() {
        return Err(format!("visitor name is empty for id_card {id_card}"));
    }

    Ok(VisitorInfo {
        id_card: id_card.to_string(),
        name,
        phone,
        photo,
        id_photo,
    })
}

pub async fn fetch_visitor_info(
    account: &str,
    id_card: &str,
) -> Result<(VisitorInfo, String), String> {
    let data = build_fetch_data(account, id_card)?;
    let data_str = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    let binding_str = BINDING_FORMULAS.to_string();
    let stamp = Utc::now().timestamp_millis().to_string();
    let referer = build_referer(account);

    let form_params = [
        ("_csrf_token", CSRF_TOKEN.to_string()),
        ("_locale_time_zone_offset", "28800000".to_string()),
        ("appType", APP_TYPE.to_string()),
        ("formUuid", FORM_UUID.to_string()),
        ("linkDataNum", "6".to_string()),
        ("bindingComponentFormulaList", binding_str),
        ("data", data_str),
    ];

    let response = reqwest::Client::new()
        .post(FETCH_URL)
        .query(&[
            ("_api", "nattyFetch"),
            ("_mock", "false"),
            ("_stamp", stamp.as_str()),
        ])
        .header("accept", ACCEPT)
        .header("accept-language", ACCEPT_LANGUAGE)
        .header("bx-v", BX_V)
        .header("dnt", "1")
        .header("origin", ORIGIN)
        .header("priority", "u=1, i")
        .header("referer", &referer)
        .header(
            "sec-ch-ua",
            "\"Not:A-Brand\";v=\"99\", \"Google Chrome\";v=\"145\", \"Chromium\";v=\"145\"",
        )
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"macOS\"")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin")
        .header("user-agent", USER_AGENT)
        .header("x-requested-with", "XMLHttpRequest")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("cookie", COOKIE)
        .header("x-csrf-token", CSRF_TOKEN)
        .form(&form_params)
        .send()
        .await
        .map_err(|e| format!("fetch visitor info failed: {e}"))?;

    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read response: {e}"))?;

    if !(200..=299).contains(&status) {
        return Err(format!("fetch visitor info: status {status}, body: {text}"));
    }

    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("failed to parse json: {e}"))?;

    let info = extract_visitor_from_response(id_card, &body)?;
    Ok((info, text))
}
