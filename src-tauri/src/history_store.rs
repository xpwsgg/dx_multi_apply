use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

const HISTORY_FILE_NAME: &str = "apply_history.json";
const RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub date: String,
    pub reception_id: String,
    pub submitted_at: String,
}

fn parse_submitted_at(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn dedup_by_key(records: Vec<HistoryRecord>) -> Vec<HistoryRecord> {
    let mut map: HashMap<String, HistoryRecord> = HashMap::new();

    for record in records {
        let key = format!("{}-{}", record.date, record.reception_id);
        match map.get(&key) {
            Some(existing) => {
                if record.submitted_at > existing.submitted_at {
                    map.insert(key, record);
                }
            }
            None => {
                map.insert(key, record);
            }
        }
    }

    let mut deduped = map.into_values().collect::<Vec<_>>();
    deduped.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
    deduped
}

fn prune_recent(records: Vec<HistoryRecord>, now: DateTime<Utc>) -> Vec<HistoryRecord> {
    let cutoff = now - Duration::days(RETENTION_DAYS);

    let filtered = records
        .into_iter()
        .filter(|record| {
            parse_submitted_at(&record.submitted_at)
                .map(|submitted_at| submitted_at >= cutoff)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    dedup_by_key(filtered)
}

fn history_file_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
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

    Ok(app_data_dir.join(HISTORY_FILE_NAME))
}

fn read_records_from_file(path: &Path) -> Result<Vec<HistoryRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read history file {}: {err}", path.display()))?;

    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str::<Vec<HistoryRecord>>(&text)
        .map_err(|err| format!("failed to parse history file {}: {err}", path.display()))
}

fn write_records_to_file(path: &Path, records: &[HistoryRecord]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(records)
        .map_err(|err| format!("failed to encode history records: {err}"))?;

    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|err| {
        format!(
            "failed to write temp history file {}: {err}",
            temp_path.display()
        )
    })?;

    if path.exists() {
        fs::remove_file(path)
            .map_err(|err| format!("failed to replace history file {}: {err}", path.display()))?;
    }

    fs::rename(&temp_path, path).map_err(|err| {
        format!(
            "failed to rename temp history file {} to {}: {err}",
            temp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

fn load_and_prune(path: &Path) -> Result<Vec<HistoryRecord>, String> {
    let records = read_records_from_file(path)?;
    Ok(prune_recent(records, Utc::now()))
}

pub fn get_recent_history(app_handle: &tauri::AppHandle) -> Result<Vec<HistoryRecord>, String> {
    let path = history_file_path(app_handle)?;
    let records = load_and_prune(&path)?;
    write_records_to_file(&path, &records)?;
    Ok(records)
}

pub fn get_existing_keys(
    app_handle: &tauri::AppHandle,
    dates: &[String],
    reception_id: &str,
) -> Result<Vec<String>, String> {
    let records = get_recent_history(app_handle)?;
    let key_set = records
        .into_iter()
        .map(|record| format!("{}-{}", record.date, record.reception_id))
        .collect::<HashSet<_>>();

    let mut existing = Vec::new();
    let mut seen = HashSet::new();
    for date in dates {
        let key = format!("{}-{}", date, reception_id);
        if key_set.contains(&key) && seen.insert(date.clone()) {
            existing.push(date.clone());
        }
    }

    Ok(existing)
}

pub fn upsert_success_record(app_handle: &tauri::AppHandle, date: &str, reception_id: &str) -> Result<(), String> {
    let path = history_file_path(app_handle)?;
    let mut records = load_and_prune(&path)?;

    records.push(HistoryRecord {
        date: date.to_string(),
        reception_id: reception_id.to_string(),
        submitted_at: Utc::now().to_rfc3339(),
    });

    let records = dedup_by_key(records);
    write_records_to_file(&path, &records)
}

#[cfg(test)]
mod tests {
    use super::{dedup_by_key, prune_recent, HistoryRecord};
    use chrono::{DateTime, Duration, Utc};

    fn make_record(date: &str, reception_id: &str, submitted_at: DateTime<Utc>) -> HistoryRecord {
        HistoryRecord {
            date: date.to_string(),
            reception_id: reception_id.to_string(),
            submitted_at: submitted_at.to_rfc3339(),
        }
    }

    #[test]
    fn should_keep_latest_when_same_date_and_reception() {
        let now = Utc::now();
        let older = make_record("2026-03-01", "emp001", now - Duration::days(2));
        let newer = make_record("2026-03-01", "emp001", now - Duration::days(1));
        let unique = make_record("2026-03-02", "emp001", now);

        let deduped = dedup_by_key(vec![older, newer.clone(), unique.clone()]);
        assert_eq!(deduped.len(), 2);
        assert!(deduped.iter().any(|item| item.date == newer.date && item.reception_id == newer.reception_id));
        assert!(deduped.iter().any(|item| item.date == unique.date && item.reception_id == unique.reception_id));
    }

    #[test]
    fn should_allow_same_date_different_reception() {
        let now = Utc::now();
        let record1 = make_record("2026-03-01", "emp001", now);
        let record2 = make_record("2026-03-01", "emp002", now);

        let deduped = dedup_by_key(vec![record1.clone(), record2.clone()]);
        assert_eq!(deduped.len(), 2);
        assert!(deduped.iter().any(|item| item.reception_id == "emp001"));
        assert!(deduped.iter().any(|item| item.reception_id == "emp002"));
    }

    #[test]
    fn should_prune_records_older_than_retention_window() {
        let now = Utc::now();
        let old = make_record("2026-01-01", "emp001", now - Duration::days(45));
        let recent = make_record("2026-03-02", "emp001", now - Duration::days(2));

        let pruned = prune_recent(vec![old, recent.clone()], now);
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].date, recent.date);
    }
}
