use std::time::{SystemTime, UNIX_EPOCH};

pub fn system_time_to_timestamp(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn unix_timestamp() -> i64 {
    system_time_to_timestamp(SystemTime::now())
}

pub fn err_to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
