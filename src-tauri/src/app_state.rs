use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct AppState {
    stop_flag: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn validate_dates(dates: &[String]) -> Result<(), String> {
    if dates.is_empty() {
        return Err("dates cannot be empty".to_string());
    }
    Ok(())
}

pub fn reset_stop(state: &tauri::State<'_, AppState>) {
    state.stop_flag.store(false, Ordering::Relaxed);
}

pub fn request_stop(state: &tauri::State<'_, AppState>) {
    state.stop_flag.store(true, Ordering::Relaxed);
}

pub fn is_stopped(state: &tauri::State<'_, AppState>) -> bool {
    state.stop_flag.load(Ordering::Relaxed)
}
