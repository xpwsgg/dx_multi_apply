#[test]
fn should_reject_empty_tasks() {
    let result = crate::app_state::validate_non_empty_task_list::<String>(&vec![]);
    assert!(result.is_err());
}
