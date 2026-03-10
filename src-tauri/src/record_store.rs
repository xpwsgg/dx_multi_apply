use rusqlite::Connection;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

const DB_FILE_NAME: &str = "records.db";

fn db_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| format!("failed to resolve app data dir: {err}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|err| {
        format!(
            "failed to create app data dir {}: {err}",
            app_data_dir.display()
        )
    })?;

    Ok(app_data_dir.join(DB_FILE_NAME))
}

fn open_db(app_handle: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app_handle)?;
    Connection::open(&path).map_err(|err| format!("failed to open database {}: {err}", path.display()))
}

pub fn init_db(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let conn = open_db(app_handle)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS submission_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            reception_id TEXT NOT NULL,
            submitted_at TEXT NOT NULL,
            UNIQUE(date, reception_id)
        );",
    )
    .map_err(|err| format!("failed to initialize database: {err}"))
}

pub fn upsert_record(
    app_handle: &tauri::AppHandle,
    date: &str,
    reception_id: &str,
) -> Result<(), String> {
    let conn = open_db(app_handle)?;
    conn.execute(
        "INSERT INTO submission_records (date, reception_id, submitted_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(date, reception_id)
         DO UPDATE SET submitted_at = datetime('now')",
        rusqlite::params![date, reception_id],
    )
    .map_err(|err| format!("failed to upsert record: {err}"))?;
    Ok(())
}

pub fn get_existing_dates(
    app_handle: &tauri::AppHandle,
    dates: &[String],
) -> Result<Vec<String>, String> {
    if dates.is_empty() {
        return Ok(Vec::new());
    }

    let conn = open_db(app_handle)?;
    let placeholders = dates.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT DISTINCT date FROM submission_records WHERE date IN ({placeholders})"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("failed to prepare query: {err}"))?;

    let params = rusqlite::params_from_iter(dates.iter());
    let rows = stmt
        .query_map(params, |row| row.get::<_, String>(0))
        .map_err(|err| format!("failed to query existing dates: {err}"))?;

    let found: HashSet<String> = rows.filter_map(|r| r.ok()).collect();

    let mut existing = Vec::new();
    let mut seen = HashSet::new();
    for date in dates {
        if found.contains(date) && seen.insert(date.clone()) {
            existing.push(date.clone());
        }
    }

    Ok(existing)
}

pub fn get_existing_keys(
    app_handle: &tauri::AppHandle,
    dates: &[String],
    reception_id: &str,
) -> Result<Vec<String>, String> {
    if dates.is_empty() {
        return Ok(Vec::new());
    }

    let conn = open_db(app_handle)?;
    let placeholders = dates.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT DISTINCT date FROM submission_records WHERE reception_id = ?1 AND date IN ({placeholders})"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("failed to prepare query: {err}"))?;

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(reception_id.to_string())];
    for date in dates {
        param_values.push(Box::new(date.clone()));
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(&*param_refs, |row| row.get::<_, String>(0))
        .map_err(|err| format!("failed to query existing keys: {err}"))?;

    let found: HashSet<String> = rows.filter_map(|r| r.ok()).collect();

    let mut existing = Vec::new();
    let mut seen = HashSet::new();
    for date in dates {
        if found.contains(date) && seen.insert(date.clone()) {
            existing.push(date.clone());
        }
    }

    Ok(existing)
}
