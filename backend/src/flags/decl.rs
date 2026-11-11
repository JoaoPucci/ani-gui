//! Flag declarations: the name, the built-in default and the values
//! the remote file may select.
//!
//! The kinds are the safety rule written as a type. A kill switch and a
//! fallback cannot name a value riskier than the build's own default:
//!
//! - a [`FlagKind::KillSwitch`] gates a feature that ships on. Its
//!   default is `true`; the only other value the file may send is
//!   `false`, which switches the feature off;
//! - a [`FlagKind::Fallback`] chooses between the behaviour this
//!   build ships (`current`, the default) and behaviours earlier
//!   releases had (`earlier`). The file may select only those;
//! - a [`FlagKind::Rollout`] gates a feature that ships off. Its
//!   default is `false`; `true` turns the feature on, which is riskier
//!   than the default, so only a file whose signature verifies may
//!   select it ([`FlagDecl::signed_only`]; the signature's
//!   [`super::signature::admit`] drops it from any other file).
//!
//! What the type cannot see is whether "off" or "earlier" really is
//! the safer side for a given feature — a kill switch on something
//! protective (a check, a warning, the update notice) would turn the
//! rule inside out. That is a review question, so every declaration
//! states its answer in `safe_because`, and [`validate`] refuses a
//! declaration that leaves it empty.

use serde::Serialize;

/// A resolved flag value. The string of a [`FlagValue::Choice`] is
/// always borrowed from a declaration, never from the remote file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum FlagValue {
    /// A kill switch: `true` while the feature runs, `false` once the
    /// file switches it off.
    Bool(bool),
    /// A fallback: the name of the behaviour in force.
    Choice(&'static str),
}

/// Which way a flag may move, and so which values the file may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagKind {
    /// Gates a feature that ships on. Default `true`; the file may
    /// send only `false`.
    KillSwitch,
    /// Chooses a behaviour. Default `current`; the file may send only
    /// one of `earlier`, behaviours a previous release had.
    Fallback {
        /// The behaviour this build ships, and the default.
        current: &'static str,
        /// Behaviours earlier releases had, which the file may
        /// select instead.
        earlier: &'static [&'static str],
    },
    /// Gates a feature that ships off. Default `false`; any file may
    /// send `false`, and only a verified file `true`.
    Rollout,
}

/// One declared flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagDecl {
    /// The key the remote file uses. Lower-case ASCII letters, digits
    /// and underscores.
    pub name: &'static str,
    /// Which way the flag may move.
    pub kind: FlagKind,
    /// Why the non-default values are the safer side: what still works,
    /// or what the user still gets, once the flag is flipped. For a
    /// rollout, why the feature may be turned on from a signed file:
    /// what it does once on, and that its off state is the build's own.
    pub safe_because: &'static str,
}

impl FlagDecl {
    /// The value used when the file says nothing usable about this
    /// flag.
    #[must_use]
    pub fn default_value(&self) -> FlagValue {
        match self.kind {
            FlagKind::KillSwitch => FlagValue::Bool(true),
            FlagKind::Fallback { current, .. } => FlagValue::Choice(current),
            FlagKind::Rollout => FlagValue::Bool(false),
        }
    }

    /// Every value this flag can resolve to, the default first.
    #[must_use]
    pub fn allowed(&self) -> Vec<FlagValue> {
        match self.kind {
            FlagKind::KillSwitch => vec![FlagValue::Bool(true), FlagValue::Bool(false)],
            FlagKind::Rollout => vec![FlagValue::Bool(false), FlagValue::Bool(true)],
            FlagKind::Fallback { current, earlier } => std::iter::once(current)
                .chain(earlier.iter().copied())
                .map(FlagValue::Choice)
                .collect(),
        }
    }

    /// The declared value `raw` selects, or `None` when `raw` is of
    /// the wrong type or names a value this flag does not allow. The
    /// value returned is one of [`FlagDecl::allowed`], never built
    /// from `raw`.
    #[must_use]
    pub fn accept(&self, raw: &serde_json::Value) -> Option<FlagValue> {
        self.allowed().into_iter().find(|v| matches(*v, raw))
    }

    /// Whether `v` may come only from a file whose signature verifies:
    /// a rollout's `true`, which turns its feature on. Every other
    /// allowed value is the default or a move toward the safer side.
    #[must_use]
    pub fn signed_only(&self, v: FlagValue) -> bool {
        self.kind == FlagKind::Rollout && v == FlagValue::Bool(true)
    }
}

/// Whether `raw` spells the declared value `v`, type included.
fn matches(v: FlagValue, raw: &serde_json::Value) -> bool {
    match v {
        FlagValue::Bool(b) => raw.as_bool() == Some(b),
        FlagValue::Choice(c) => raw.as_str() == Some(c),
    }
}

/// Check a declaration list: names well-formed and unique, every
/// fallback naming at least one earlier behaviour distinct from its
/// current one, and every declaration stating why its other values are
/// safe (for a rollout, why a signed file may turn it on).
///
/// # Errors
/// A description of the first problem found.
pub fn validate(decls: &[FlagDecl]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for decl in decls {
        check_one(decl)?;
        if !seen.insert(decl.name) {
            return Err(format!("{} is declared twice", decl.name));
        }
    }
    Ok(())
}

fn check_one(decl: &FlagDecl) -> Result<(), String> {
    let name = decl.name;
    let well_formed = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !well_formed {
        return Err(format!("{name:?} is not a lower-case snake_case name"));
    }
    if decl.safe_because.trim().is_empty() {
        return Err(format!("{name} does not say why its other values are safe"));
    }
    if let FlagKind::Fallback { current, earlier } = decl.kind {
        check_fallback(name, current, earlier)?;
    }
    Ok(())
}

fn check_fallback(name: &str, current: &str, earlier: &[&str]) -> Result<(), String> {
    if earlier.is_empty() {
        return Err(format!("{name} names no earlier behaviour to fall back to"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for value in std::iter::once(current).chain(earlier.iter().copied()) {
        if value.is_empty() || !seen.insert(value) {
            return Err(format!("{name} has an empty or repeated value {value:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "decl_test.rs"]
mod tests;
