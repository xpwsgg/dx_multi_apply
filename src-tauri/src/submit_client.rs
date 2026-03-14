use chrono::{NaiveDate, Utc};
use serde_json::Value;

use crate::http_common::{
    build_referer, yida_client, ACCEPT, ACCEPT_LANGUAGE, APP_TYPE, BX_V, COOKIE, CSRF_TOKEN, FORM_UUID, ORIGIN,
    SCHEMA_VERSION, USER_AGENT,
};
use crate::reception_client::ReceptionInfo;
use crate::request_template::build_payload;
#[cfg(test)]
use crate::request_template::build_payload_for_date;
use crate::visitor_client::VisitorInfo;

const SUBMIT_URL: &str = "https://iw68lh.aliwork.com/o/HW9663A19D6M1QDL6D7GNAO1L2ZC2NBXQHOXL3";

#[derive(Debug)]
pub struct SubmitResult {
    pub status_code: u16,
    pub response_text: String,
}

#[derive(Debug)]
pub struct SubmitError {
    pub message: String,
    pub response_raw: Option<String>,
}

#[cfg(test)]
pub trait HttpClient {
    fn post_form(&self, _date_text: &str, _encoded_value: &str) -> (u16, String);
}

#[cfg(test)]
pub struct FakeHttp {
    status_code: u16,
    body: String,
}

#[cfg(test)]
impl FakeHttp {
    pub fn new(status_code: u16, body: &str) -> Self {
        Self {
            status_code,
            body: body.to_string(),
        }
    }
}

#[cfg(test)]
impl HttpClient for FakeHttp {
    fn post_form(&self, _date_text: &str, _encoded_value: &str) -> (u16, String) {
        (self.status_code, self.body.clone())
    }
}

fn validate_business_success(status_code: u16, response_text: &str) -> Result<(), SubmitError> {
    if !(200..=299).contains(&status_code) {
        return Err(SubmitError {
            message: format!("non-success status: {status_code}, body: {response_text}"),
            response_raw: Some(response_text.to_string()),
        });
    }

    let body: Value = serde_json::from_str(response_text).map_err(|err| SubmitError {
        message: format!("failed to parse response json: {err}, body: {response_text}"),
        response_raw: Some(response_text.to_string()),
    })?;

    if body.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(SubmitError {
            message: format!("business failed: success is not true, body: {response_text}"),
            response_raw: Some(response_text.to_string()),
        });
    }

    Ok(())
}

/// Test-only: uses the hardcoded template data (build_payload_for_date).
#[cfg(test)]
pub async fn submit_once_with_client(
    client: &impl HttpClient,
    date_text: &str,
) -> Result<SubmitResult, SubmitError> {
    let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|err| SubmitError {
        message: err.to_string(),
        response_raw: None,
    })?;

    let payload = build_payload_for_date(date).map_err(|err| SubmitError {
        message: err,
        response_raw: None,
    })?;

    let encoded_value = serde_json::to_string(&payload).map_err(|err| SubmitError {
        message: err.to_string(),
        response_raw: None,
    })?;

    let (status_code, response_text) = client.post_form(date_text, &encoded_value);
    validate_business_success(status_code, &response_text)?;

    Ok(SubmitResult {
        status_code,
        response_text,
    })
}

pub async fn submit_once(
    account: &str,
    visitors: &[VisitorInfo],
    reception: &ReceptionInfo,
    date: NaiveDate,
) -> Result<SubmitResult, SubmitError> {
    let payload = build_payload(date, account, visitors, reception).map_err(|err| SubmitError {
        message: err,
        response_raw: None,
    })?;

    let encoded_value = serde_json::to_string(&payload).map_err(|err| SubmitError {
        message: err.to_string(),
        response_raw: None,
    })?;

    let form_params = [
        ("_csrf_token", CSRF_TOKEN.to_string()),
        ("formUuid", FORM_UUID.to_string()),
        ("appType", APP_TYPE.to_string()),
        ("value", encoded_value),
        ("_schemaVersion", SCHEMA_VERSION.to_string()),
    ];
    let stamp = Utc::now().timestamp_millis().to_string();
    let referer = build_referer(account);

    let response = yida_client()
        .post(SUBMIT_URL)
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
        .map_err(|err| SubmitError {
            message: format!("request failed: {err}"),
            response_raw: None,
        })?;

    let status_code = response.status().as_u16();
    let response_text = response.text().await.map_err(|err| SubmitError {
        message: format!("failed to read response body: {err}"),
        response_raw: None,
    })?;

    validate_business_success(status_code, &response_text)?;

    Ok(SubmitResult {
        status_code,
        response_text,
    })
}
