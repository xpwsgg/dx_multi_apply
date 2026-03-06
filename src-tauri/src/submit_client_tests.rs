#[tokio::test]
async fn should_mark_non_2xx_as_failure() {
    let fake = crate::submit_client::FakeHttp::new(500, "internal error");
    let result = crate::submit_client::submit_once_with_client(&fake, "2026-03-08").await;
    assert!(result.is_err());
}
