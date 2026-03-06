use chrono::NaiveDate;

use crate::request_template::build_payload_for_date;

const SUBMIT_URL: &str = "https://iw68lh.aliwork.com";
const COOKIE: &str = "cookie-placeholder";
const CSRF_TOKEN: &str = "csrf-placeholder";
const FORM_UUID: &str = "form-uuid-placeholder";
const APP_TYPE: &str = "APP_WUKONG";
const SCHEMA_VERSION: &str = "2.0";

#[derive(Debug)]
pub struct SubmitResult {
    pub status_code: u16,
    pub response_text: String,
}

#[derive(Debug)]
pub struct SubmitError {
    pub message: String,
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

pub async fn submit_once_with_client(
    client: &impl HttpClient,
    date_text: &str,
) -> Result<SubmitResult, SubmitError> {
    let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|err| SubmitError {
            message: err.to_string(),
        })?;

    let payload = build_payload_for_date(date).map_err(|err| SubmitError { message: err })?;
    let encoded_value = serde_json::to_string(&payload).map_err(|err| SubmitError {
        message: err.to_string(),
    })?;

    let _ = (
        SUBMIT_URL,
        COOKIE,
        CSRF_TOKEN,
        FORM_UUID,
        APP_TYPE,
        SCHEMA_VERSION,
    );

    let (status_code, response_text) = client.post_form(date_text, &encoded_value);
    if !(200..=299).contains(&status_code) {
        return Err(SubmitError {
            message: format!("non-success status: {status_code}, body: {response_text}"),
        });
    }

    Ok(SubmitResult {
        status_code,
        response_text,
    })
}

pub async fn submit_once(_date: NaiveDate) -> Result<SubmitResult, SubmitError> {
    Err(SubmitError {
        message: "submit_once is not implemented yet".to_string(),
    })
}
