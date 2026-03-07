#[tokio::test]
async fn should_mark_non_2xx_as_failure() {
    let fake = crate::submit_client::FakeHttp::new(500, "internal error");
    let result = crate::submit_client::submit_once_with_client(&fake, "2026-03-08").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("non-success status"));
    assert_eq!(err.response_raw.as_deref(), Some("internal error"));
}

#[tokio::test]
async fn should_mark_success_false_as_failure() {
    let fake = crate::submit_client::FakeHttp::new(200, r#"{"success":false,"errorMsg":"bad"}"#);
    let result = crate::submit_client::submit_once_with_client(&fake, "2026-03-08").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("success is not true"));
    assert_eq!(
        err.response_raw.as_deref(),
        Some(r#"{"success":false,"errorMsg":"bad"}"#)
    );
}

#[tokio::test]
async fn should_mark_invalid_json_as_failure() {
    let fake = crate::submit_client::FakeHttp::new(200, "ok");
    let result = crate::submit_client::submit_once_with_client(&fake, "2026-03-08").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("failed to parse response json"));
    assert_eq!(err.response_raw.as_deref(), Some("ok"));
}

#[tokio::test]
async fn should_pass_when_success_true() {
    let fake = crate::submit_client::FakeHttp::new(200, r#"{"success":true,"data":{}}"#);
    let result = crate::submit_client::submit_once_with_client(&fake, "2026-03-08").await;

    assert!(result.is_ok());
    let submit_result = result.unwrap();
    assert_eq!(submit_result.status_code, 200);
    assert_eq!(submit_result.response_text, r#"{"success":true,"data":{}}"#);
}
