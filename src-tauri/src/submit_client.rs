use chrono::{NaiveDate, Utc};
use serde_json::Value;

use crate::request_template::build_payload_for_date;

const SUBMIT_URL: &str = "https://iw68lh.aliwork.com/o/HW9663A19D6M1QDL6D7GNAO1L2ZC2NBXQHOXL3";
const ORIGIN: &str = "https://iw68lh.aliwork.com";
const REFERER: &str = "https://iw68lh.aliwork.com/o/fk_ybfk?account=17849759601&company=%E5%BA%86%E9%BC%8E%E7%B2%BE%E5%AF%86%E7%94%B5%E5%AD%90(%E6%B7%AE%E5%AE%89)%E6%9C%89%E9%99%90%E5%85%AC%E5%8F%B8&part=%E6%B7%AE%E5%AE%89%E7%AC%AC%E4%BA%8C%E5%9B%AD%E5%8C%BA&applyType=%E4%B8%80%E8%88%AC%E8%AE%BF%E5%AE%A2";
const COOKIE: &str = "isg=BGpqwESq96zc_ntA73hqkgqyu9YM2-41v5mXHfQhLL1IJw_h3G_pRd0UslM7s2bN; tianshu_corp_user=ding2b4c83bec54a29c6f2c783f7214b6d69_FREEUSER; tianshu_csrf_token=c5683320-e1de-4fc0-b89d-65b268eaacd1; c_csrf=c5683320-e1de-4fc0-b89d-65b268eaacd1; cookie_visitor_id=WfkHnTNp; tianshu_app_type=APP_GRVPTEOQ6D4B7FLZFYNJ; JSESSIONID=872E09D38EDC3118067499E5A0303485";
const CSRF_TOKEN: &str = "c5683320-e1de-4fc0-b89d-65b268eaacd1";
const FORM_UUID: &str = "FORM-2768FF7B2C0D4A0AB692FD28DBA09FD57IHQ";
const APP_TYPE: &str = "APP_GRVPTEOQ6D4B7FLZFYNJ";
const SCHEMA_VERSION: &str = "669";
const ACCEPT: &str = "application/json, text/json";
const ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,ja-JP;q=0.8,ja;q=0.7";
const BX_V: &str = "2.5.11";
const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";

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

pub trait HttpClient {
    fn post_form(&self, _date_text: &str, _encoded_value: &str) -> (u16, String);
}

pub struct FakeHttp {
    status_code: u16,
    body: String,
}

impl FakeHttp {
    pub fn new(status_code: u16, body: &str) -> Self {
        Self {
            status_code,
            body: body.to_string(),
        }
    }
}

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

pub async fn submit_once(date: NaiveDate) -> Result<SubmitResult, SubmitError> {
    let payload = build_payload_for_date(date).map_err(|err| SubmitError {
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

    let response = reqwest::Client::new()
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
        .header("referer", REFERER)
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
