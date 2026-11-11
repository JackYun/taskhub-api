//! Request / response DTOs, database row types and validation helpers.

pub mod datetime;
pub mod health;
pub mod task;
pub mod user;

use std::borrow::Cow;

use serde::{Deserialize, Deserializer};
use validator::ValidationError;

/// PATCH helper for nullable fields (use with `#[serde(default)]`):
/// absent -> `None` (unchanged), `null` -> `Some(None)` (clear),
/// value -> `Some(Some(v))` (set).
pub(crate) fn nullable<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

/// Build a `ValidationError` with a code and a human-readable message.
pub(crate) fn invalid(
    code: &'static str,
    message: impl Into<Cow<'static, str>>,
) -> ValidationError {
    let mut err = ValidationError::new(code);
    err.message = Some(message.into());
    err
}

/// Rejects strings that are empty after trimming whitespace.
pub(crate) fn validate_not_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        Err(invalid("blank", "must not be blank"))
    } else {
        Ok(())
    }
}
