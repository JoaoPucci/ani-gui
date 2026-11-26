//! A release's number, as the tips' windows count them: `X.Y.Z`.
//!
//! Only the three numbers count. A suffix after them (`-dev`, `-rc.1`,
//! `+build`) is not part of a tip's window, so `0.19.0-dev` reads as
//! `0.19.0`; anything else is not a release.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A release number. Ordered by major, then minor, then patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Release {
    /// The major number.
    pub major: u32,
    /// The minor number.
    pub minor: u32,
    /// The patch number.
    pub patch: u32,
}

impl Release {
    /// The release `major.minor.patch`.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// The release `text` names: three dot-separated numbers, with an
    /// optional `v` before them and an optional suffix after them that
    /// starts with `-` or `+`. `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let core = text.split(['-', '+']).next()?;
        let mut parts = core.split('.');
        let mut number = || -> Option<u32> {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse().ok()
        };
        let release = Self::new(number()?, number()?, number()?);
        parts.next().is_none().then_some(release)
    }

    /// The first release `n` minor releases after this one's minor:
    /// `0.19.4` plus two is `0.21.0`. Saturates rather than wrapping.
    #[must_use]
    pub const fn plus_minors(self, n: u32) -> Self {
        Self::new(self.major, self.minor.saturating_add(n), 0)
    }
}

impl fmt::Display for Release {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Serialize for Release {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Release {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("{text:?} is not a release number")))
    }
}

#[cfg(test)]
#[path = "release_test.rs"]
mod tests;
