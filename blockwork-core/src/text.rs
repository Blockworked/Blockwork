//! Text and date helpers behind the text/data operators and `SplitIntoList`.

use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

/// Compiled patterns by source, since a loop re-evaluates the same one.
static REGEX_CACHE: Mutex<Option<HashMap<String, Regex>>> = Mutex::new(None);
const REGEX_CACHE_MAX: usize = 64;

pub fn regex(pattern: &str) -> Result<Regex, String> {
    let mut guard = REGEX_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(re) = cache.get(pattern) {
        return Ok(re.clone());
    }
    let re = Regex::new(pattern).map_err(|e| format!("bad regex: {}", e.to_string().lines().last().unwrap_or("invalid")))?;
    if cache.len() >= REGEX_CACHE_MAX {
        cache.clear();
    }
    cache.insert(pattern.to_string(), re.clone());
    Ok(re)
}

/// `contains`, `starts with`, `ends with` or `matches regex`.
pub fn check(text: &str, mode: &str, other: &str) -> Result<bool, String> {
    Ok(match mode {
        "starts with" => text.starts_with(other),
        "ends with" => text.ends_with(other),
        "matches regex" => regex(other)?.is_match(text),
        _ => text.contains(other),
    })
}

pub fn regex_replace(pattern: &str, text: &str, replacement: &str) -> Result<String, String> {
    Ok(regex(pattern)?.replace_all(text, replacement).into_owned())
}

/// The first match, or its first group when the pattern has one; empty
/// when nothing matches.
pub fn regex_find(pattern: &str, text: &str) -> Result<String, String> {
    let re = regex(pattern)?;
    let Some(caps) = re.captures(text) else { return Ok(String::new()) };
    let found = if re.captures_len() > 1 { caps.get(1) } else { caps.get(0) };
    Ok(found.map(|m| m.as_str().to_string()).unwrap_or_default())
}

/// `text` cut at every `separator`; an empty separator splits into letters.
pub fn split(text: &str, separator: &str) -> Vec<String> {
    if separator.is_empty() {
        return text.chars().map(String::from).collect();
    }
    text.split(separator).map(String::from).collect()
}

/// Part `index` (from 1; negative counts from the end) of `text` split by `separator`.
pub fn split_part(text: &str, separator: &str, index: f64) -> Result<String, String> {
    let parts = split(text, separator);
    let n = index.round() as i64;
    let at = if n < 0 { parts.len() as i64 + n } else { n - 1 };
    usize::try_from(at)
        .ok()
        .and_then(|i| parts.get(i))
        .cloned()
        .ok_or_else(|| format!("there's no part {n} ({} parts)", parts.len()))
}

/// Letters `from` to `to` (inclusive, from 1) of `text`, clamped to its length.
pub fn substring(text: &str, from: f64, to: f64) -> String {
    let len = text.chars().count() as i64;
    let from = (from.round() as i64).max(1);
    let to = (to.round() as i64).min(len);
    if from > to {
        return String::new();
    }
    text.chars().skip((from - 1) as usize).take((to - from + 1) as usize).collect()
}

/// `n` with exactly `decimals` digits after the point.
pub fn format_number(n: f64, decimals: f64) -> String {
    let decimals = decimals.round().clamp(0.0, 20.0) as usize;
    format!("{n:.decimals$}")
}

pub fn now_timestamp() -> f64 {
    let now = chrono::Utc::now();
    now.timestamp() as f64 + f64::from(now.timestamp_subsec_millis()) / 1000.0
}

fn local(timestamp: f64) -> Result<DateTime<Local>, String> {
    let secs = timestamp.floor();
    let nanos = ((timestamp - secs) * 1e9).round().clamp(0.0, 999_999_999.0) as u32;
    Local.timestamp_opt(secs as i64, nanos).single().ok_or_else(|| format!("{timestamp} isn't a valid timestamp"))
}

/// Formats a Unix timestamp in local time with strftime codes like `%Y-%m-%d`.
pub fn format_date(timestamp: f64, format: &str) -> Result<String, String> {
    let items: Vec<Item> = StrftimeItems::new(format).collect();
    if items.iter().any(|i| matches!(i, Item::Error)) {
        return Err(format!("\"{format}\" isn't a valid date format (use codes like %Y-%m-%d %H:%M)"));
    }
    Ok(local(timestamp)?.format_with_items(items.into_iter()).to_string())
}

const AUTO_FORMATS: [&str; 5] = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y/%m/%d %H:%M"];

/// The Unix timestamp of a local date written in `format`, or in a common
/// format when it's empty.
pub fn parse_date(text: &str, format: &str) -> Result<f64, String> {
    let text = text.trim();
    let naive = if format.trim().is_empty() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
            return Ok(dt.timestamp() as f64);
        }
        AUTO_FORMATS
            .iter()
            .find_map(|f| NaiveDateTime::parse_from_str(text, f).ok())
            .or_else(|| ["%Y-%m-%d", "%Y/%m/%d"].iter().find_map(|f| NaiveDate::parse_from_str(text, f).ok()).and_then(|d| d.and_hms_opt(0, 0, 0)))
    } else {
        NaiveDateTime::parse_from_str(text, format)
            .ok()
            .or_else(|| NaiveDate::parse_from_str(text, format).ok().and_then(|d| d.and_hms_opt(0, 0, 0)))
    };
    let naive = naive.ok_or_else(|| format!("couldn't read \"{text}\" as a date"))?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.timestamp() as f64)
        .ok_or_else(|| format!("\"{text}\" doesn't exist in the local time zone"))
}

/// One part of a timestamp in local time: year, month, day, hour, minute,
/// second, or weekday (1 = Sunday, like `current day of week`).
pub fn date_part(timestamp: f64, part: &str) -> Result<f64, String> {
    let dt = local(timestamp)?;
    Ok(f64::from(match part {
        "year" => return Ok(f64::from(dt.year())),
        "month" => dt.month(),
        "day" => dt.day(),
        "hour" => dt.hour(),
        "minute" => dt.minute(),
        "second" => dt.second(),
        "weekday" => dt.weekday().number_from_sunday(),
        other => return Err(format!("unknown date part \"{other}\"")),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_tools() {
        assert!(check("hello world", "contains", "lo w").unwrap());
        assert!(check("hello", "starts with", "he").unwrap() && !check("hello", "ends with", "he").unwrap());
        assert!(check("order 1234", "matches regex", r"\d{4}$").unwrap());
        assert!(check("x", "matches regex", "(").unwrap_err().starts_with("bad regex"));
        assert_eq!(regex_replace(r"(\w+)@(\w+)", "me@host", "$2 at $1").unwrap(), "host at me");
        assert_eq!(regex_find(r"#(\d+)", "issue #42 and #7").unwrap(), "42");
        assert_eq!(regex_find(r"\d+", "abc").unwrap(), "");
        assert_eq!(split("a,b,,c", ","), ["a", "b", "", "c"]);
        assert_eq!(split("abc", ""), ["a", "b", "c"]);
        assert_eq!(split_part("a,b,c", ",", 2.0).unwrap(), "b");
        assert_eq!(split_part("a,b,c", ",", -1.0).unwrap(), "c");
        assert!(split_part("a,b", ",", 3.0).is_err());
        assert_eq!(substring("héllo", 2.0, 4.0), "éll");
        assert_eq!(substring("abc", 3.0, 10.0), "c");
        assert_eq!(substring("abc", 3.0, 1.0), "");
        assert_eq!(format_number(3.14159, 2.0), "3.14");
        assert_eq!(format_number(2.0, 0.0), "2");
    }

    #[test]
    fn dates_round_trip_in_local_time() {
        let ts = parse_date("2026-03-04 05:06:07", "").unwrap();
        assert_eq!(format_date(ts, "%Y-%m-%d %H:%M:%S").unwrap(), "2026-03-04 05:06:07");
        assert_eq!(date_part(ts, "month").unwrap(), 3.0);
        assert_eq!(date_part(ts, "weekday").unwrap(), 4.0, "a Wednesday");
        assert_eq!(parse_date("04.03.2026", "%d.%m.%Y").unwrap(), parse_date("2026-03-04", "").unwrap());
        assert_eq!(parse_date("2026-03-04T05:06:07Z", "").unwrap(), 1_772_600_767.0);
        assert!(parse_date("tomorrow", "").is_err());
        assert!(format_date(ts, "%Q").is_err());
        assert!(now_timestamp() > 1.7e9);
    }
}
