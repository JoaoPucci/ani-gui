//! The rules the registry holds to ([`validate`]): ids and places
//! well-formed and unique, and each tip's own declaration sound. The
//! shape of each tool, its window and its walkthrough are in
//! [`super::rules_shape`].

use std::collections::BTreeSet;

use super::decl::TipDecl;
use super::rules_shape::{check_how_to, check_shape, check_window};

/// Check a registry: ids well-formed and unique, places well-formed
/// and unique within a tip, each tool's shape, windows that open before
/// they close, flags this build declares, and walkthroughs that name a
/// sequence on a page of their own.
///
/// # Errors
/// A description of the first problem found.
pub fn validate(tips: &[TipDecl], flags: &[crate::flags::FlagDecl]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for tip in tips {
        check_one(tip, flags)?;
        if !ids.insert(tip.id.as_str()) {
            return Err(format!("{} is declared twice", tip.id));
        }
    }
    tips.iter().try_for_each(|t| check_how_to(t, tips))
}

/// Lower-case ASCII words joined by single `-`.
fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn check_one(tip: &TipDecl, flags: &[crate::flags::FlagDecl]) -> Result<(), String> {
    let id = &tip.id;
    if !is_kebab(id) {
        return Err(format!("{id:?} is not lower-case words joined by -"));
    }
    check_places(tip)?;
    check_shape(tip)?;
    check_window(tip)?;
    if let Some(flag) = &tip.flag {
        if !flags.iter().any(|d| d.name == flag) {
            return Err(format!(
                "{id} names {flag}, which this build does not declare"
            ));
        }
    }
    Ok(())
}

fn check_places(tip: &TipDecl) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for place in &tip.places {
        if !is_kebab(place) {
            return Err(format!("{}: place {place:?} is not kebab-case", tip.id));
        }
        if !seen.insert(place) {
            return Err(format!("{}: place {place} is named twice", tip.id));
        }
    }
    Ok(())
}
