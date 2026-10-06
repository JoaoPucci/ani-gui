//! What the renderer may name as a Kitsu id.
//!
//! Kitsu ids are digits. The renderer only ever holds ones Kitsu or
//! this backend handed it, but every route it reaches is a loopback
//! HTTP surface, and a value taken from it ends up in cache keys
//! (`kitsu:v5:anime:<id>`, `availability:v14:<id>:<mode>`,
//! `airing:v2:<id>`), in database rows, and in the path of an
//! outbound Kitsu request. A value that is not an id — `../49877`,
//! `49877/x`, `12:21` — would name a different row or a different
//! upstream resource than the one it claims to.
//!
//! So the routes decide at the boundary, once, with one rule: the
//! value trimmed of surrounding whitespace, when it is non-empty and
//! all ASCII digits, is the id; anything else is not one. A route
//! whose id is required answers [`AniError::InvalidKitsuId`]; a field
//! where the id is optional reads a non-id as no id at all, which is
//! what each of those paths already does when the renderer sends none.

use serde::{Deserialize, Deserializer};

use crate::error::AniError;

/// The digits `raw` carries, or `None` when it is not a Kitsu id.
#[must_use]
pub fn kitsu_id_in(raw: &str) -> Option<&str> {
    // Red: every value passes, as every route takes one today.
    Some(raw)
}

/// The id a route requires, or the error the route answers with.
///
/// # Errors
/// [`AniError::InvalidKitsuId`] when `raw` is not a Kitsu id.
pub fn require(raw: &str) -> Result<&str, AniError> {
    kitsu_id_in(raw).ok_or(AniError::Metadata)
}

/// Serde adapter for an optional id field: the id when the value is
/// one, `None` when it is absent, empty, or anything else.
///
/// # Errors
/// Propagates from the underlying deserializer if the field is not a
/// string or null.
pub fn deserialize_optional<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(d)?;
    Ok(raw)
}

/// Serde adapter for a list of ids: the ids among the values, each
/// trimmed, in order; a value that is not one is dropped.
///
/// # Errors
/// Propagates from the underlying deserializer if the field is not a
/// list of strings.
pub fn deserialize_list<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<String>::deserialize(d)?;
    Ok(raw)
}

#[cfg(test)]
#[path = "kitsu_id_test.rs"]
mod tests;
