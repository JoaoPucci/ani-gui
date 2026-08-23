//! The availability probes' pick walk — the search-and-pick half of
//! the native resolution walk, split from `play_native_resolve` so
//! each file stays inside the complexity ratchet's per-file bar.

use crate::error::AniError;
use crate::scraper::anidb::{AnidbClient, AnidbFetch};

use super::play_native::{pick_candidate, PickedShow};
use super::play_native_resolve::NativeError;

/// The walk's pick half, shared with the availability probes: search
/// canonical then fallbacks, pick per pool, and classify the miss —
/// the same alias-recovery, walk-stopping, and clean-miss semantics
/// as [`resolve_native`], without the episode chain. Gate admission
/// is the transport's job here exactly as there: callers pass a
/// client whose fetch is gated.
///
/// # Errors
/// [`NativeError`] with `clean_miss` set only for the
/// all-clean-no-match verdict.
pub async fn pick_native_walk<F: AnidbFetch>(
    client: &AnidbClient<F>,
    title: &str,
    alt_titles: &[String],
    expected_count: Option<u32>,
    year: Option<u32>,
    subtype: Option<&str>,
) -> std::result::Result<PickedShow, NativeError> {
    let mut any_search_succeeded = false;
    let mut any_search_errored = false;
    let mut any_answered_dead_end = false;
    let mut last_failure_at: Option<tokio::time::Instant> = None;
    for t in std::iter::once(title).chain(alt_titles.iter().map(String::as_str)) {
        match client.search(t).await {
            Ok(hits) => {
                any_search_succeeded = true;
                if hits.is_empty() {
                    continue;
                }
                match pick_candidate(client, &hits, expected_count, t, year, subtype).await {
                    Ok(picked) => return Ok(picked),
                    // A rejected pool is a clean verdict about THIS
                    // pool; the next alias may carry the real show.
                    Err(AniError::NoResults) => {}
                    Err(e) if e.is_provider_block() || matches!(e, AniError::GateRefused) => {
                        return Err(NativeError {
                            error: e,
                            clean_miss: false,
                            failed_at: None,
                        });
                    }
                    // All-answered-not-found: dead candidates on a
                    // healthy provider; the walk moves on.
                    Err(AniError::Upstream { .. }) => {
                        any_answered_dead_end = true;
                    }
                    Err(_) => {
                        any_search_errored = true;
                        last_failure_at = Some(
                            client
                                .transport()
                                .last_attempt_at()
                                .unwrap_or_else(tokio::time::Instant::now),
                        );
                    }
                }
            }
            Err(AniError::Upstream { status }) => {
                return Err(NativeError {
                    error: AniError::Upstream { status },
                    clean_miss: false,
                    failed_at: None,
                });
            }
            Err(AniError::GateRefused) => {
                return Err(NativeError {
                    error: AniError::GateRefused,
                    clean_miss: false,
                    failed_at: None,
                });
            }
            Err(_) => {
                any_search_errored = true;
                last_failure_at = Some(
                    client
                        .transport()
                        .last_attempt_at()
                        .unwrap_or_else(tokio::time::Instant::now),
                );
            }
        }
    }
    if !any_search_succeeded || any_search_errored {
        return Err(NativeError {
            error: AniError::Network,
            clean_miss: false,
            failed_at: last_failure_at,
        });
    }
    if any_answered_dead_end {
        return Err(NativeError {
            error: AniError::NoResults,
            clean_miss: false,
            failed_at: None,
        });
    }
    Err(NativeError {
        error: AniError::NoResults,
        clean_miss: true,
        failed_at: None,
    })
}

#[cfg(test)]
#[path = "play_native_walk_test.rs"]
mod tests;
