use serde::{Deserialize, Serialize};
use serde_json::Value;

const SEARCH_URL: &str =
    "https://dingtalk.avaryholding.com:8443/dingplus/searchFormData";
const RECEPTION_FORM_UUID: &str = "FORM-B965E22437E1415BBBBA33011BF20FB54VP8";
const RECEPTION_APP_TYPE: &str = "APP_GRVPTEOQ6D4B7FLZFYNJ";
const RECEPTION_SYSTEM_TOKEN: &str = "DC666GC1PN6LT8C7C64FD9N62P2E3F9V1SFWLKQ61";

const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceptionInfo {
    pub employee_id: String,
    pub name: String,
    pub department: String,
    pub phone: String,
}

fn extract_reception_from_response(
    employee_id: &str,
    body: &Value,
) -> Result<ReceptionInfo, String> {
    let records = body
        .pointer("/body/data")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing body.data in response".to_string())?;

    let record = records
        .first()
        .ok_or_else(|| format!("no reception record found for employee {employee_id}"))?;

    let form_data = record
        .get("formData")
        .ok_or_else(|| "missing formData in record".to_string())?;

    let name = form_data
        .get("textField_m3pkk1ez")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let department = form_data
        .get("textField_m3pgo9p1")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let phone = form_data
        .get("textField_m3pollg0")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    if name.is_empty() {
        return Err(format!(
            "reception name is empty for employee {employee_id}"
        ));
    }

    Ok(ReceptionInfo {
        employee_id: employee_id.to_string(),
        name,
        department,
        phone,
    })
}

pub async fn fetch_reception_info(employee_id: &str) -> Result<(ReceptionInfo, String), String> {
    let json_inner = format!(
        "{{\"employeeField_m3o6fym4\": [\"{}\"]}}",
        employee_id
    );

    let request_body = serde_json::json!({
        "formUUid": RECEPTION_FORM_UUID,
        "appType": RECEPTION_APP_TYPE,
        "systemToken": RECEPTION_SYSTEM_TOKEN,
        "json": json_inner,
    });

    let response = reqwest::Client::new()
        .post(SEARCH_URL)
        .header("accept", "application/json, text/json")
        .header("accept-language", "zh-CN,zh;q=0.9,ja-JP;q=0.8,ja;q=0.7")
        .header("content-type", "application/json")
        .header("dnt", "1")
        .header("origin", "https://iw68lh.aliwork.com")
        .header("referer", "https://iw68lh.aliwork.com/")
        .header(
            "sec-ch-ua",
            "\"Not:A-Brand\";v=\"99\", \"Google Chrome\";v=\"145\", \"Chromium\";v=\"145\"",
        )
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", "\"macOS\"")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "cross-site")
        .header("sec-fetch-storage-access", "active")
        .header("user-agent", USER_AGENT)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("fetch reception info failed: {e}"))?;

    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .map_err(|e| format!("failed to read response: {e}"))?;

    if !(200..=299).contains(&status) {
        return Err(format!(
            "fetch reception info: status {status}, body: {text}"
        ));
    }

    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("failed to parse json: {e}"))?;

    let info = extract_reception_from_response(employee_id, &body)?;
    Ok((info, text))
}
