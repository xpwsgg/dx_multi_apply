use chrono::NaiveDate;
use std::sync::{Arc, Mutex};

struct FakeSubmitter {
    fail_on_call: usize,
    calls: Arc<Mutex<Vec<NaiveDate>>>,
}

impl FakeSubmitter {
    fn new(fail_on_call: usize) -> Self {
        Self {
            fail_on_call,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn submitted_dates(&self) -> Vec<NaiveDate> {
        self.calls.lock().unwrap().clone()
    }
}

impl crate::batch_runner::BatchSubmitter for FakeSubmitter {
    fn submit_date(&self, date: NaiveDate) -> Result<(), String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(date);
        if calls.len() == self.fail_on_call {
            return Err("submit failed".to_string());
        }
        Ok(())
    }
}

#[tokio::test]
async fn should_submit_in_ascending_order_and_stop_on_first_failure() {
    let dates = vec![
        NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
        NaiveDate::from_ymd_opt(2026, 3, 7).unwrap(),
        NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(),
    ];

    let submitter = FakeSubmitter::new(2);
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let result = crate::batch_runner::run_batch_dates(dates, &submitter, stop_flag).await;

    assert!(result.is_err());
    assert_eq!(
        submitter.submitted_dates(),
        vec![
            NaiveDate::from_ymd_opt(2026, 3, 7).unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(),
        ]
    );
}
