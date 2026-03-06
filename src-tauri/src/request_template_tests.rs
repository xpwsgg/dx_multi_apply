#[test]
fn should_replace_visit_date_and_visit_date_text_consistently() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
    let payload = crate::request_template::build_payload_for_date(date).unwrap();

    let date_value = crate::request_template::extract_field_value(&payload, "到访日期").unwrap();
    let date_text = crate::request_template::extract_field_value(&payload, "到访日期文本").unwrap();

    assert_eq!(date_text, "2026-03-08");
    assert_eq!(date_value, "1772899200000");
}
