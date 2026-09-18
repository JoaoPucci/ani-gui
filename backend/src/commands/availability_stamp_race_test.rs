//! What a replay's stamp carries forward when a native resolve
//! writes the same row while the replay is on its way to it.
//!
//! The two writers are ordered by the row lock alone. The refresh
//! generation does not separate them: only a cache-bypassing refresh
//! bumps it, so a resolve's stamp leaves it where it was and the
//! replay behind it still passes `with_row_if_ours`. Whatever the
//! replay writes there lands on top of the resolve's row and holds
//! for a fresh lifetime, which is why it has to be the row as it
//! stands at the moment of the write rather than the one the replay
//! happened to see earlier.
//!
//! Mounted by `#[path]` beside the availability tests and borrowing
//! their state builder.

use super::tests::cache_only_state;
use super::*;
use crate::scraper::provider::ProviderId;

/// The show both writers stamp, in the mode a resolve's cap is exact
/// for.
const ID: &str = "K9";
const MODE: &str = "sub";

fn positive_row(provider: ProviderId, cap: Option<u32>) -> AvailabilityResponse {
    AvailabilityResponse {
        available: true,
        episode_count: cap,
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        provider: Some(provider),
    }
}

/// The row a probe left behind before either writer ran.
fn seed_standing_row(state: &AppState, provider: ProviderId, cap: u32) {
    write_cache_full(state, ID, MODE, None, &positive_row(provider, Some(cap)));
}

/// The row as the next reader would find it — still within its
/// lifetime, or there is nothing to compare.
fn row_now(state: &AppState) -> AvailabilityResponse {
    let body = meta_cache_get(&state.cache_pool, &cache_key(ID, MODE))
        .expect("the cache reads")
        .expect("the row is within its lifetime");
    serde_json::from_str(&body).expect("the row parses")
}

/// Stage the interleaving deterministically: the test takes the row
/// first, so both writers queue for it, and tokio's mutex hands it
/// over in the order they asked. The resolve asks first and so
/// writes first; the replay asks second, but it is polled — and on
/// the current code reads the row — while the resolve's write is
/// still ahead of it in the queue. Then the row is released and the
/// two writes run back to back, the replay's last.
async fn a_resolve_writes_while_the_replay_waits_for_the_row(
    state: &AppState,
    resolved_by: ProviderId,
    resolved_cap: u32,
    replayed_from: ProviderId,
) {
    let key = cache_key(ID, MODE);
    let generation = state.availability_refreshes.generation(&key);
    let held = state
        .availability_refreshes
        .for_row(&key)
        .lock_owned()
        .await;

    let resolve = tokio::spawn({
        let state = state.clone();
        async move {
            stamp_after_native(
                &state,
                Some(ID),
                MODE,
                generation,
                ResolveVerdict::served(resolved_by, Some(resolved_cap), &[]),
            )
            .await;
        }
    });
    tokio::task::yield_now().await;

    let replay = tokio::spawn({
        let state = state.clone();
        async move {
            stamp_after_cache_hit(&state, Some(ID), MODE, generation, replayed_from).await;
        }
    });
    tokio::task::yield_now().await;

    drop(held);
    resolve.await.expect("the resolve's stamp finishes");
    replay.await.expect("the replay's stamp finishes");
}

/// A resolve that paid for the listing wrote the exact cap onto the
/// row while the replay was queued behind it. The replay learned
/// nothing about the listing — it served a stream the resolution
/// cache already held — so the row it refreshes is the one the
/// resolve just wrote, not the count the replay read on its way in.
/// Writing that older count back would re-gate the episodes the
/// resolve had just unlocked, for the row's whole lifetime.
#[tokio::test]
async fn a_replay_keeps_the_cap_a_resolve_wrote_while_it_waited_for_the_row() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Anidb, 12);

    a_resolve_writes_while_the_replay_waits_for_the_row(
        &state,
        ProviderId::Anidb,
        24,
        ProviderId::Anidb,
    )
    .await;

    let row = row_now(&state);
    assert!(row.available, "the show is still carried");
    assert_eq!(
        row.episode_count,
        Some(24),
        "the resolve's exact cap stands through the replay's refresh"
    );
    assert_eq!(row.provider, Some(ProviderId::Anidb));
}

/// The same ordering where the resolve failed over: the standing row
/// named the primary, the resolve reached the show through the
/// fallback and stamped the fallback's affinity and cap, and the
/// replay — serving a cached row that same fallback resolved —
/// writes last. The row the replay read named the primary and said
/// nothing about the fallback's listing; the row it writes is the
/// fallback's, cap included, so the next play starts where the
/// resolve proved the show is.
#[tokio::test]
async fn a_replay_keeps_the_row_a_failed_over_resolve_wrote_while_it_waited() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Anidb, 12);

    a_resolve_writes_while_the_replay_waits_for_the_row(
        &state,
        ProviderId::Hianime,
        24,
        ProviderId::Hianime,
    )
    .await;

    let row = row_now(&state);
    assert_eq!(
        row.provider,
        Some(ProviderId::Hianime),
        "the fallback's affinity stands"
    );
    assert_eq!(
        row.episode_count,
        Some(24),
        "with the cap the fallback's listing paid for"
    );
}

/// No race at all, only a replay through a provider other than the
/// one the standing row names: the show failed over to hianime, its
/// positive row carries hianime's exact cap, and the user replays an
/// episode the resolution cache still holds from anidb.app's days.
/// A served replay proves nothing about anidb.app's listing or its
/// health now — the CDN answered for a URL resolved long ago — so
/// the row it refreshes is the one that stands, hianime's, cap and
/// affinity kept. Writing a cap-less anidb.app row over it would
/// send the next uncached episode back to the provider that failed
/// over and lose the cap the fallback's listing paid for.
#[tokio::test]
async fn a_replay_through_another_provider_leaves_the_standing_positive_row_as_it_is() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Hianime, 24);
    let generation = state
        .availability_refreshes
        .generation(&cache_key(ID, MODE));

    stamp_after_cache_hit(&state, Some(ID), MODE, generation, ProviderId::Anidb).await;

    let row = row_now(&state);
    assert_eq!(
        row.provider,
        Some(ProviderId::Hianime),
        "the standing row's affinity is not moved by a replay through another provider"
    );
    assert_eq!(
        row.episode_count,
        Some(24),
        "and its exact cap is carried forward"
    );
}

/// A probe that set out with no positive row to remember runs the
/// primary alone, and the primary answers a clean miss. While it
/// was out, a resolve reached the show through the fallback and
/// stamped hianime's positive row with its exact cap. The probe's
/// miss is older than that row and proves nothing against it —
/// anidb.app never had the show — so the row it finds standing when
/// it takes the lock is the one that stands: the miss is the
/// verdict the caller sees, and nothing is persisted over the row.
/// Written as the primary's negative, the row would hide a stream
/// just proven playable for the negative's whole lifetime.
#[tokio::test]
async fn a_probes_clean_miss_does_not_overwrite_a_positive_row_stamped_while_it_was_out() {
    use wiremock::matchers::{method, path};
    let anidb = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"<div class="grid"><p>No results.</p></div>"#)
                .set_delay(std::time::Duration::from_millis(300)),
        )
        .mount(&anidb)
        .await;
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb];
    let state = std::sync::Arc::new(state);

    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": MODE,
        "kitsu_id": ID
    }))
    .expect("args");
    let key = cache_key(ID, MODE);
    let generation = state.availability_refreshes.generation(&key);

    let probe = tokio::spawn({
        let state = state.clone();
        let base = anidb.uri();
        async move { check_availability_with_base(&state, &args, Some(&base)).await }
    });
    // The resolve lands while the probe waits on the primary.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        generation,
        ResolveVerdict::served(ProviderId::Hianime, Some(24), &[]),
    )
    .await;
    let got = probe.await.expect("the probe finishes");

    assert!(
        matches!(got, Err(crate::error::AniError::NoResults)),
        "the primary's miss is the verdict the caller sees: {got:?}"
    );
    let row = row_now(&state);
    assert!(row.available, "the fallback's positive row stands");
    assert_eq!(row.provider, Some(ProviderId::Hianime));
    assert_eq!(
        row.episode_count,
        Some(24),
        "with the cap the resolve wrote"
    );
}
