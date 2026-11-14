//! UTC times as the contract writes them, `date -u +%FT%TZ`, without a calendar library.

use std::time::{SystemTime, UNIX_EPOCH};

/// Days since 1970-01-01 of a civil date (proleptic Gregorian), as `gdd/limits.sh` computes them.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let yy = if m <= 2 { y - 1 } else { y };
    let era = if yy >= 0 { yy } else { yy - 399 } / 400;
    let yoe = yy - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Seconds since the epoch of the first 19 characters of a time, `YYYY-MM-DDTHH:MM:SS`, read as UTC.
/// Like the counter, it ignores what follows: fractions, a `Z` or an offset.
pub fn parse(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if !digits.iter().all(|&i| b[i].is_ascii_digit())
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let n = |a: usize, z: usize| s[a..z].parse::<i64>().ok();
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (hh, mm, ss) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    Some(days_from_civil(y, m, d) * 86400 + hh * 3600 + mm * 60 + ss)
}

/// Whether a time is written exactly the way the contract asks: `YYYY-MM-DDTHH:MM:SSZ`.
pub fn is_contract_form(s: &str) -> bool {
    s.len() == 20 && s.ends_with('Z') && parse(s).is_some()
}

pub fn format(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let rest = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The clock: `GDD_NOW` when it is set (for tests and for replaying a log), else the system's.
pub fn now() -> i64 {
    if let Ok(v) = std::env::var("GDD_NOW")
        && let Some(t) = parse(&v)
    {
        return t;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now_string() -> String {
    format(now())
}
/// How long ago, in words a person reads: "3 minutes", "2 hours 5 minutes", "1 day 4 hours".
pub fn waited(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    let unit = |n: i64, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    if d > 0 {
        format!("{} {}", unit(d, "day"), unit(h, "hour"))
    } else if h > 0 {
        format!("{} {}", unit(h, "hour"), unit(m, "minute"))
    } else {
        unit(m, "minute")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for s in [
            "1970-01-01T00:00:00Z",
            "2026-11-10T10:05:00Z",
            "2000-02-29T23:59:59Z",
            "2100-03-01T00:00:00Z",
        ] {
            assert_eq!(format(parse(s).unwrap()), s);
        }
        assert_eq!(parse("2026-11-10T10:05:00Z").unwrap(), 1794305100);
    }

    #[test]
    fn reads_like_the_counter() {
        assert_eq!(
            parse("2026-11-10T10:05:00.123+02:00"),
            parse("2026-11-10T10:05:00Z")
        );
        assert_eq!(parse("2026-11-10 10:05:00Z"), None);
        assert!(!is_contract_form("2026-11-10T10:05:00.1Z"));
        assert!(is_contract_form("2026-11-10T10:05:00Z"));
    }

    #[test]
    fn words() {
        assert_eq!(waited(59), "0 minutes");
        assert_eq!(waited(61), "1 minute");
        assert_eq!(waited(3 * 3600 + 120), "3 hours 2 minutes");
        assert_eq!(waited(86400 + 3600), "1 day 1 hour");
    }
}
