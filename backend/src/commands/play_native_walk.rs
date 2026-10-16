//! The availability probes' pick walk — the search-and-pick half of
//! the native resolution walk, split from `play_native_resolve` so
//! each file stays inside the complexity ratchet's per-file bar.

use crate::error::AniError;
use crate::scraper::provider::Provider;

use super::play_native::{pick_candidate_titled, PickedShow};
use super::play_native_resolve::NativeError;
use super::play_native_transient::{attempt_at, Transients, WalkStage};

/// The walk's pick half, shared with the availability probes and the
/// range download's start: search canonical then fallbacks, pick per
/// pool, and classify the miss — the same alias-recovery,
/// walk-stopping, clean-miss and transient-verdict semantics as
/// [`resolve_native`], without the episode chain: a transient failure
/// is reported as what it was ([`Transients`]), never substituted.
/// Gate admission is the transport's job here exactly as there:
/// callers pass a client whose fetch is gated.
///
/// # Errors
/// [`NativeError`] with `clean_miss` set only for the
/// all-clean-no-match verdict.
pub async fn pick_native_walk<P: Provider + ?Sized>(
    client: &P,
    title: &str,
    alt_titles: &[String],
    expected_count: Option<u32>,
    year: Option<u32>,
    subtype: Option<&str>,
    tail: Option<&dyn super::play_native_appended::TailSource>,
) -> std::result::Result<PickedShow, NativeError> {
    let entry_titles: Vec<&str> = std::iter::once(title)
        .chain(alt_titles.iter().map(String::as_str))
        .collect();
    let mut transients = Transients::default();
    let mut any_answered_dead_end = false;
    for t in std::iter::once(title).chain(alt_titles.iter().map(String::as_str)) {
        match client.search(t).await {
            Ok(hits) => {
                if hits.is_empty() {
                    continue;
                }
                match pick_candidate_titled(
                    client,
                    &hits,
                    expected_count,
                    t,
                    &entry_titles,
                    year,
                    subtype,
                )
                .await
                {
                    Ok(picked) => {
                        return Ok(super::play_native_appended::fit_appended(
                            picked,
                            expected_count,
                            tail,
                        )
                        .await)
                    }
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
                    Err(e) => {
                        transients.note(WalkStage::Pick, e, attempt_at(client));
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
            Err(e) => {
                transients.note(WalkStage::Search, e, attempt_at(client));
            }
        }
    }
    if let Some(ne) = transients.verdict() {
        return Err(ne);
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

#[cfg(test)]
#[path = "play_native_walk_fixture_test.rs"]
mod fixture_tests;
