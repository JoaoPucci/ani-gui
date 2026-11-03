//! The app-wide watch-order setting: `"linked"` (the order Kitsu's
//! prequel and sequel links give, the default) or `"released"` (start
//! dates). Split from `config/mod.rs` like `syncplay.rs`.

/// The order the links give.
pub const LINKED: &str = "linked";
/// The order the entries started in.
pub const RELEASED: &str = "released";

/// The setting a stored value reads as: `"released"` as itself, and
/// anything else, an unknown value a newer build wrote included, as
/// `"linked"`.
#[must_use]
pub fn normalize(value: &str) -> &'static str {
    let _ = value;
    LINKED
}

/// Read the stored value as [`normalize`] does.
pub(super) fn read<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <String as serde::Deserialize>::deserialize(deserializer)?;
    Ok(value)
}

#[cfg(test)]
#[path = "watch_order_prop_test.rs"]
mod prop_tests;
