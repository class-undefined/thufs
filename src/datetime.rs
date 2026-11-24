use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};

// Display dates at the requested Shanghai UTC+8 offset, independently of the
// machine timezone and the offset supplied by the server.
const SHANGHAI_SECONDS: i32 = 8 * 60 * 60;
const DISPLAY_FORMAT: &str = "%Y-%m-%d %H:%M:%S %:z";

pub fn from_timestamp(timestamp: i64) -> Option<String> {
    let zone = FixedOffset::east_opt(SHANGHAI_SECONDS)?;
    Some(
        DateTime::<Utc>::from_timestamp(timestamp, 0)?
            .with_timezone(&zone)
            .format(DISPLAY_FORMAT)
            .to_string(),
    )
}

fn normalized(value: &str) -> Option<String> {
    let value = value.trim();
    let zone = FixedOffset::east_opt(SHANGHAI_SECONDS)?;
    if let Ok(timestamp) = value.parse::<i64>() {
        return from_timestamp(timestamp);
    }
    if let Ok(date) =
        DateTime::parse_from_rfc3339(value).or_else(|_| DateTime::parse_from_rfc2822(value))
    {
        return Some(date.with_timezone(&zone).format(DISPLAY_FORMAT).to_string());
    }
    for format in ["%Y-%m-%d %H:%M:%S%.f %:z", "%Y-%m-%d %H:%M:%S%.f UTC%:z"] {
        if let Ok(date) = DateTime::parse_from_str(value, format) {
            return Some(date.with_timezone(&zone).format(DISPLAY_FORMAT).to_string());
        }
    }
    if let Some(value) = value.strip_suffix(" UTC") {
        let date = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f").ok()?;
        return from_timestamp(date.and_utc().timestamp());
    }
    // Unzoned Seafile timestamps use server-local Shanghai time.
    for format in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
        if let Ok(date) = NaiveDateTime::parse_from_str(value, format) {
            return Some(
                zone.from_local_datetime(&date)
                    .single()?
                    .format(DISPLAY_FORMAT)
                    .to_string(),
            );
        }
    }
    None
}

pub fn display(value: &str) -> String {
    normalized(value).unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_timestamps_and_offsets_to_shanghai_across_date_boundaries() {
        assert_eq!(
            from_timestamp(1704067200).as_deref(),
            Some("2024-01-01 08:00:00 +08:00")
        );
        assert_eq!(
            display("2023-12-31T20:30:00Z"),
            "2024-01-01 04:30:00 +08:00"
        );
        assert_eq!(
            display("2024-01-01T00:30:00+09:00"),
            "2023-12-31 23:30:00 +08:00"
        );
        assert_eq!(
            display("2024-01-01 00:00:00 UTC"),
            "2024-01-01 08:00:00 +08:00"
        );
        assert!(from_timestamp(i64::MAX).is_none());
    }

    #[test]
    fn local_and_already_formatted_dates_keep_the_same_instant() {
        assert_eq!(display("2026-11-24 10:15:05"), "2026-11-24 10:15:05 +08:00");
        assert_eq!(
            display("2026-11-24T10:15:05+08:00"),
            "2026-11-24 10:15:05 +08:00"
        );
        assert_eq!(
            display("2026-11-24 10:15:05 +08:00"),
            "2026-11-24 10:15:05 +08:00"
        );
        assert_eq!(display("2026-11-24"), "2026-11-24");
        assert_eq!(display("—"), "—");
    }
}
