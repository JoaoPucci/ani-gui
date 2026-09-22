//! What a replay's stamp leaves standing when a native resolve
//! writes the same row while the replay is on its way to it.
//!
//! The two writers are ordered by the row lock alone. The refresh
//! generation does not separate them: only a cache-bypassing refresh
//! bumps it, so a resolve's stamp leaves it where it was and the
//! replay behind it still passes `with_row_if_ours`. Whatever the
//! replay decides there it decides over the resolve's row, which is
//! why it has to read the row as it stands at the moment of the
//! write rather than the one it happened to see earlier — and why a
//! positive row it finds standing is left as it is, lifetime and
//! all, rather than written back.
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

/// The moment the row was last written, as the cache stamps it: the
/// stamp its lifetime runs from.
fn written_at(state: &AppState) -> i64 {
    let conn = state
        .cache_pool
        .get()
        .expect("the cache pool lends a connection");
    conn.query_row(
        "SELECT fetched_at FROM meta_cache WHERE key = ?1",
        [cache_key(ID, MODE)],
        |r| r.get(0),
    )
    .expect("the row is there")
}

/// Move the row's stamp back by `secs`, so a write that renews it
/// shows even inside the second the test runs in.
fn age_row(state: &AppState, secs: i64) {
    let conn = state
        .cache_pool
        .get()
        .expect("the cache pool lends a connection");
    let changed = conn
        .execute(
            "UPDATE meta_cache SET fetched_at = fetched_at - ?1 WHERE key = ?2",
            rusqlite::params![secs, cache_key(ID, MODE)],
        )
        .expect("the row's stamp moves");
    assert_eq!(changed, 1, "the row to age is at {}", cache_key(ID, MODE));
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
    let at_start = RowAtStart::read(state, Some(ID), MODE);
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
                &at_start,
                ResolveVerdict::served(resolved_by, Some(resolved_cap), &[]),
            )
            .await;
        }
    });
    tokio::task::yield_now().await;

    let replay = tokio::spawn({
        let state = state.clone();
        async move {
            stamp_after_cache_hit(&state, Some(ID), MODE, &at_start, replayed_from).await;
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
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_cache_hit(&state, Some(ID), MODE, &at_start, ProviderId::Anidb).await;

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
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

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
        &at_start,
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

/// A replay of a still-live cached episode, with the show's positive
/// row standing. The replay validated an old CDN URL and learned
/// nothing about the listing, so the row keeps its stamp along with
/// its cap: written back with a fresh one, an ongoing show's exact
/// cap, replayed daily, would never reach the reprobe that learns
/// its new episodes, and every episode past the old cap would stay
/// gated for as long as the user kept replaying the ones before it.
#[tokio::test]
async fn a_replay_leaves_a_standing_positive_row_its_own_lifetime() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Hianime, 24);
    age_row(&state, 3600);
    let stamped_at = written_at(&state);
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_cache_hit(&state, Some(ID), MODE, &at_start, ProviderId::Hianime).await;

    assert_eq!(
        written_at(&state),
        stamped_at,
        "the standing row keeps the moment it was written, and with it the lifetime it had left"
    );
    let row = row_now(&state);
    assert_eq!(row.provider, Some(ProviderId::Hianime));
    assert_eq!(row.episode_count, Some(24));
}

/// Two resolves for the same show set out together with no positive
/// row to remember, so both run the primary first. One finds it
/// unreachable, fails over, and stamps the fallback's success —
/// hianime's row, exact cap and all. The other is answered late by
/// the primary with a clean miss. That miss was earned without the
/// fallback's proof: it says the primary never had the show, which
/// the fallback's row does not dispute, and written over that row
/// it would hide a stream just proven playable, and take the
/// affinity with it — every walk after it would start from the
/// primary and end on the same miss. The row that stands when the
/// miss comes to be written is the one that stands.
#[tokio::test]
async fn a_native_miss_does_not_overwrite_a_positive_row_stamped_after_it_set_out() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::served(ProviderId::Hianime, Some(24), &[]),
    )
    .await;
    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::missed(Some(ProviderId::Anidb)),
    )
    .await;

    let row = row_now(&state);
    assert!(row.available, "the fallback's success stands: {row:?}");
    assert_eq!(row.provider, Some(ProviderId::Hianime));
    assert_eq!(
        row.episode_count,
        Some(24),
        "with the cap the fallback's listing paid for"
    );
}

/// The same, from a standing row: the primary's positive row stood
/// when both set out, and one resolve — the primary unreachable to
/// it — reached the show through the fallback and moved the row
/// there. The other's late miss was measured against the primary's
/// row, the one it set out from, and never saw the fallback's; the
/// fallback's stands.
#[tokio::test]
async fn a_native_miss_does_not_overwrite_a_positive_row_that_moved_after_it_set_out() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Anidb, 12);
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::served(ProviderId::Hianime, Some(24), &[]),
    )
    .await;
    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::missed(Some(ProviderId::Anidb)),
    )
    .await;

    let row = row_now(&state);
    assert!(row.available, "the fallback's success stands: {row:?}");
    assert_eq!(row.provider, Some(ProviderId::Hianime));
    assert_eq!(row.episode_count, Some(24));
}

/// The other half of the replay's rule: a positive row that has run
/// out is not a standing one. The resolution cache outlives the
/// availability row, and a replay served after the row's lifetime
/// writes the count-less positive row a served resolve without a
/// cap writes — the affinity the next walk starts from, and a row
/// the next look at the show reprobes for its cap.
#[tokio::test]
async fn a_replay_past_the_rows_lifetime_writes_the_providers_positive_row_again() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Hianime, 24);
    age_row(
        &state,
        i64::try_from(AVAILABILITY_TTL_ONGOING_SECS).expect("fits") + 1,
    );
    assert!(
        meta_cache_get(&state.cache_pool, &cache_key(ID, MODE))
            .expect("the cache reads")
            .is_none(),
        "the seeded row has run out"
    );
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_cache_hit(&state, Some(ID), MODE, &at_start, ProviderId::Hianime).await;

    let row = row_now(&state);
    assert!(row.available);
    assert_eq!(
        row.provider,
        Some(ProviderId::Hianime),
        "the affinity is written again"
    );
    assert_eq!(
        row.episode_count, None,
        "count-less: the replay learned nothing about the listing"
    );
}

/// The other half of the miss's rule: measured against the very row
/// it set out from, a clean miss writes. The walk read that row's
/// provider and asked it first, set its miss aside and asked the
/// rest, so the negative weighed everything the row said; nothing
/// newer stands in its way.
#[tokio::test]
async fn a_native_miss_over_the_row_it_set_out_from_writes_the_negative() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Hianime, 24);
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::missed(Some(ProviderId::Hianime)),
    )
    .await;

    let row = row_now(&state);
    assert!(!row.available, "the miss is written: {row:?}");
    assert_eq!(row.provider, Some(ProviderId::Hianime));
}

/// The primary's positive row stood when both resolves set out. One
/// resolved through the primary and wrote the row again exactly as
/// it was — same provider, same cap, same tags — while the other was
/// still waiting on the primary; the other's late clean miss then
/// finds the row standing as it stood. It was written again, though,
/// on proof newer than the miss, so it stands and the miss is the
/// verdict the caller saw. Judged by its bytes alone the row looks
/// untouched, and the miss would disable a title just proven
/// playable for the negative's whole lifetime.
#[tokio::test]
async fn a_native_miss_does_not_overwrite_a_positive_row_written_again_as_it_stood() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    seed_standing_row(&state, ProviderId::Anidb, 12);
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::served(ProviderId::Anidb, Some(12), &[]),
    )
    .await;
    stamp_after_native(
        &state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::missed(Some(ProviderId::Anidb)),
    )
    .await;

    let row = row_now(&state);
    assert!(row.available, "the row written again stands: {row:?}");
    assert_eq!(row.provider, Some(ProviderId::Anidb));
    assert_eq!(row.episode_count, Some(12));
}

/// The probe's side of the same case: a cache-bypassing probe sets
/// out from the primary's standing row, and while its walk waits on
/// the primary's clean miss a resolve writes that very row again,
/// unchanged. The probe's negative is refused as it would be over a
/// row that changed: the row was proven again after the walk began.
#[tokio::test]
async fn a_probes_clean_miss_does_not_overwrite_a_positive_row_written_again_while_it_was_out() {
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
    seed_standing_row(&state, ProviderId::Anidb, 12);

    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Standing Show",
        "mode": MODE,
        "kitsu_id": ID,
        "bypass_cache": true
    }))
    .expect("args");
    let at_start = RowAtStart::read(&state, Some(ID), MODE);

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
        &at_start,
        ResolveVerdict::served(ProviderId::Anidb, Some(12), &[]),
    )
    .await;
    let got = probe.await.expect("the probe finishes");

    assert!(
        matches!(got, Err(crate::error::AniError::NoResults)),
        "the primary's miss is the verdict the caller sees: {got:?}"
    );
    let row = row_now(&state);
    assert!(row.available, "the row written again stands: {row:?}");
    assert_eq!(row.episode_count, Some(12));
}
