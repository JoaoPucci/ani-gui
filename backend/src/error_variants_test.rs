//! One value of every `AniError` variant, for the tests that need to
//! walk them all: the envelope test, the key-list test.
//!
//! Rust cannot enumerate an enum's variants, so the list is written out
//! — but written once, as pattern/value pairs. `every_variant!` turns
//! the patterns into a `match` with no wildcard and the values into the
//! list, so a variant added to the enum does not compile until it has a
//! pair here, and a pair cannot name a pattern without giving its
//! value. `every_value_matches_its_own_pattern` closes one gap the
//! compiler leaves: a value that is not the variant its pattern names.
//! The other is a field that chooses the key: a variant whose `key()`
//! arms split on a field (`Scraper`'s failure) needs one pair per
//! choice, and nothing but this convention says so — a `{ .. }` pair
//! over a field `key()` later starts splitting on keeps compiling, and
//! the new key reaches neither `error-keys.json` nor the catalogues.

use crate::error::{AniError, DownloadToolFailure};

/// A listed value beside a predicate for the pattern it was listed under.
type Pair = (AniError, fn(&AniError) -> bool);

macro_rules! every_variant {
    ($($pat:pat => $val:expr),* $(,)?) => {{
        // Exhaustive by construction; never called.
        #[allow(dead_code)]
        fn exhaustive(e: &AniError) {
            match e {
                $($pat => {}),*
            }
        }
        let pairs: Vec<Pair> = vec![
            $(($val, |e: &AniError| matches!(e, $pat))),*
        ];
        pairs
    }};
}

fn pairs() -> Vec<Pair> {
    every_variant! {
        AniError::Scraper { failure: DownloadToolFailure::Exited } => AniError::Scraper {
            failure: DownloadToolFailure::Exited,
        },
        AniError::Scraper { failure: DownloadToolFailure::NotStarted } => AniError::Scraper {
            failure: DownloadToolFailure::NotStarted,
        },
        AniError::Timeout => AniError::Timeout,
        AniError::NoResults => AniError::NoResults,
        AniError::EpisodeUnavailable => AniError::EpisodeUnavailable,
        AniError::RateLimited { .. } => AniError::RateLimited {
            retry_after_secs: Some(9),
        },
        AniError::ParseFailed { .. } => AniError::ParseFailed { detail: "x".into() },
        AniError::FfmpegMissing => AniError::FfmpegMissing,
        AniError::Upstream { .. } => AniError::Upstream { status: 503 },
        AniError::Network => AniError::Network,
        AniError::GateRefused => AniError::GateRefused,
        AniError::PlayerSpawnFailed { .. } => AniError::PlayerSpawnFailed {
            binary: "vlc".into(),
        },
        AniError::SyncplaySpawnFailed { .. } => AniError::SyncplaySpawnFailed {
            binary: "syncplay".into(),
        },
        AniError::Cache => AniError::Cache,
        AniError::Io => AniError::Io,
        AniError::Config => AniError::Config,
        AniError::Metadata => AniError::Metadata,
        AniError::UnsupportedPkce => AniError::UnsupportedPkce,
        AniError::InvalidKitsuId => AniError::InvalidKitsuId,
        AniError::InvalidSearchOffset => AniError::InvalidSearchOffset,
        AniError::InvalidRequestBody => AniError::InvalidRequestBody,
        AniError::InvalidToken => AniError::InvalidToken,
    }
}

/// One value of every variant (and of every key-choosing field value).
pub(crate) fn every_variant() -> Vec<AniError> {
    pairs().into_iter().map(|(e, _)| e).collect()
}

#[test]
fn every_value_matches_its_own_pattern() {
    for (e, is_listed_as) in pairs() {
        assert!(
            is_listed_as(&e),
            "{e:?} is not the variant its pattern names"
        );
    }
}
