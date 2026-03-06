use chrono::{FixedOffset, NaiveDate, TimeZone};

pub fn to_date_text(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn to_midnight_timestamp_ms(date: NaiveDate) -> i64 {
    let timezone = FixedOffset::east_opt(8 * 3600).expect("valid timezone offset");
    let local_midnight = date.and_hms_opt(0, 0, 0).expect("valid date midnight");
    timezone
        .from_local_datetime(&local_midnight)
        .single()
        .expect("ambiguous local datetime")
        .timestamp_millis()
}
