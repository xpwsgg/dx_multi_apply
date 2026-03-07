#[test]
fn should_reject_empty_dates() {
    let result = crate::app_state::validate_dates(&vec![]);
    assert!(result.is_err());
}
