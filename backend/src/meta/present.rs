//! A field a metadata reply must carry, though it may carry null.
//!
//! serde reads a missing `Option` field as `None`, so a reply without
//! the field its answer lives in reads the same as one whose answer is
//! "nothing": AniList's `Media: null` and a reply with no `Media` at
//! all. The first is an answer, the second is no answer. [`Present`]
//! keeps them apart: a missing field fails to deserialize, a null one
//! is `None`.

use serde::{Deserialize, Deserializer};

/// A field that must be in the reply: `None` when it is null, an error
/// when it is missing.
#[derive(Debug)]
pub(crate) struct Present<T>(pub(crate) Option<T>);

impl<T> Present<T> {
    /// The field's value, `None` when the reply gave it as null.
    pub(crate) fn into_inner(self) -> Option<T> {
        self.0
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Present<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Through a value, not `Option::deserialize`: serde hands a
        // missing field to an option as `None`, but to a value as the
        // missing-field error.
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self(None));
        }
        T::deserialize(value)
            .map(|t| Self(Some(t)))
            .map_err(serde::de::Error::custom)
    }
}

/// [`Present`] for a field kept as a plain `Option`: named by
/// `#[serde(deserialize_with = "present")]`, which leaves a missing
/// field the missing-field error where a bare `Option` would read it as
/// `None`. Null is `None`. One reading for both forms: this goes
/// through [`Present`].
pub(crate) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    Present::<T>::deserialize(deserializer).map(Present::into_inner)
}

#[cfg(test)]
#[path = "present_prop_test.rs"]
mod prop_tests;
