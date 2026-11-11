//! Flexible date parsing: accepts either a full RFC 3339 timestamp
//! (`2026-12-01T17:00:00Z`, `2026-12-01T17:00:00+08:00`) or a plain date
//! (`2026-12-01`, interpreted as 00:00:00 UTC). Values are normalised to UTC.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{de::Error as _, Deserialize, Deserializer};

/// Parse an RFC 3339 timestamp or a `YYYY-MM-DD` date.
pub fn parse_flexible(input: &str) -> Result<DateTime<Utc>, String> {
    let input = input.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        if let Some(dt) = date.and_hms_opt(0, 0, 0) {
            return Ok(dt.and_utc());
        }
    }
    Err(format!(
        "invalid date '{input}': expected RFC 3339 (2026-12-01T17:00:00Z) or YYYY-MM-DD"
    ))
}

/// `Option<DateTime<Utc>>` field: absent/null -> `None`.
pub fn option<'de, D>(de: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(de)?
        .map(|s| parse_flexible(&s).map_err(D::Error::custom))
        .transpose()
}

/// PATCH semantics (use together with `#[serde(default)]`):
/// absent -> `None` (leave unchanged), `null` -> `Some(None)` (clear),
/// value -> `Some(Some(v))` (set).
pub fn double_option<'de, D>(de: D) -> Result<Option<Option<DateTime<Utc>>>, D::Error>
where
    D: Deserializer<'de>,
{
    option(de).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_and_dates() {
        let a = parse_flexible("2026-12-01T17:00:00+08:00").unwrap();
        assert_eq!(a.to_rfc3339(), "2026-12-01T09:00:00+00:00");
        let b = parse_flexible("2026-12-01").unwrap();
        assert_eq!(b.to_rfc3339(), "2026-12-01T00:00:00+00:00");
        assert!(parse_flexible("tomorrow").is_err());
    }
}
