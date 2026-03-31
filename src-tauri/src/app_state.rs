use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct AppState {
    stop_flag: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn validate_non_empty_task_list<T>(tasks: &[T]) -> Result<(), String> {
    if tasks.is_empty() {
        return Err("任务不能为空".to_string());
    }
    Ok(())
}

/// Try to acquire the running lock. Returns `Err` if already running.
pub fn try_start(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    let was_running = state.running.swap(true, Ordering::SeqCst);
    if was_running {
        return Err("任务正在执行中，请勿重复提交".to_string());
    }
    state.stop_flag.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn finish(state: &tauri::State<'_, AppState>) {
    state.running.store(false, Ordering::SeqCst);
}

pub fn request_stop(state: &tauri::State<'_, AppState>) {
    state.stop_flag.store(true, Ordering::SeqCst);
}

pub fn is_stopped(state: &tauri::State<'_, AppState>) -> bool {
    state.stop_flag.load(Ordering::SeqCst)
}
