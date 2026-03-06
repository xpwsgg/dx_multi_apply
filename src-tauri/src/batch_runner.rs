use chrono::NaiveDate;
use rand::Rng;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub trait BatchSubmitter {
    fn submit_date(&self, date: NaiveDate) -> Result<(), String>;
}

pub async fn run_batch_dates(
    mut dates: Vec<NaiveDate>,
    submitter: &impl BatchSubmitter,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    dates.sort_unstable();

    for (index, date) in dates.into_iter().enumerate() {
        if stop_flag.load(Ordering::Relaxed) {
            return Err("batch stopped manually".to_string());
        }

        submitter.submit_date(date)?;

        if index > 0 {
            // no-op branch to keep loop shape explicit
        }

        if !stop_flag.load(Ordering::Relaxed) {
            let wait_seconds = rand::thread_rng().gen_range(120..=180);
            maybe_sleep(wait_seconds).await;
        }
    }

    Ok(())
}

#[cfg(not(test))]
async fn maybe_sleep(wait_seconds: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(wait_seconds)).await;
}

#[cfg(test)]
async fn maybe_sleep(_wait_seconds: u64) {}
