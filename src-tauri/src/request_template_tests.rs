#[test]
fn should_replace_visit_date_and_visit_date_text_consistently() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
    let payload = crate::request_template::build_payload_for_date(date).unwrap();

    let date_value = crate::request_template::extract_field_value(&payload, "到访日期").unwrap();
    let date_text = crate::request_template::extract_field_value(&payload, "到访日期文本").unwrap();

    assert_eq!(date_text, "2026-03-08");
    assert_eq!(date_value, "1772899200000");
}

fn build_test_visitor() -> crate::visitor_client::VisitorInfo {
    crate::visitor_client::VisitorInfo {
        id_card: "320882198910054470".to_string(),
        name: "肖鹏".to_string(),
        phone: "17849759601".to_string(),
        photo: serde_json::json!([]),
        id_photo: serde_json::json!([]),
        social_proof: serde_json::json!([]),
    }
}

fn build_test_reception(employee_id: &str) -> crate::reception_client::ReceptionInfo {
    crate::reception_client::ReceptionInfo {
        employee_id: employee_id.to_string(),
        name: "测试接待人".to_string(),
        department: "测试部门".to_string(),
        phone: "15861762355".to_string(),
    }
}

fn find_field<'a>(payload: &'a serde_json::Value, label: &str) -> &'a serde_json::Value {
    payload
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field.get("label").and_then(serde_json::Value::as_str) == Some(label))
        .unwrap()
}

#[test]
fn should_override_visit_area_for_special_reception() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    let visitor = build_test_visitor();
    let reception = build_test_reception("52091191");
    let payload =
        crate::request_template::build_payload(date, "17849759601", &[visitor], &reception)
            .unwrap();

    let visit_area = find_field(&payload, "到访区域");
    assert_eq!(
        visit_area
            .pointer("/fieldData/value")
            .and_then(serde_json::Value::as_str),
        Some("进入制造现场")
    );
    assert_eq!(
        visit_area
            .pointer("/fieldData/text")
            .and_then(serde_json::Value::as_str),
        Some("进入车间/管制区域")
    );
    assert_eq!(
        visit_area
            .pointer("/options/0/value")
            .and_then(serde_json::Value::as_str),
        Some("进入制造现场")
    );
    assert_eq!(
        visit_area
            .pointer("/options/0/text")
            .and_then(serde_json::Value::as_str),
        Some("进入车间/管制区域")
    );
}

#[test]
fn should_keep_default_visit_area_for_other_receptions() {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    let visitor = build_test_visitor();
    let reception = build_test_reception("12345678");
    let payload =
        crate::request_template::build_payload(date, "17849759601", &[visitor], &reception)
            .unwrap();

    let visit_area = find_field(&payload, "到访区域");
    assert_eq!(
        visit_area
            .pointer("/fieldData/value")
            .and_then(serde_json::Value::as_str),
        Some("生产区域外围（不进入制造现场）")
    );
    assert_eq!(
        visit_area
            .pointer("/fieldData/text")
            .and_then(serde_json::Value::as_str),
        Some("外围公共区域")
    );
}
