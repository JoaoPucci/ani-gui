//! The mark-watched write read through the show's episode map, against
//! a tracker held in memory: which entries are written, with what, in
//! what order — Ranma ½'s two AniList entries, a prologue AniList
//! counts, Haruhi 2009's unexplained surplus, Tower of God's later part
//! filed under its prequel's entry, and the rewatch and monotonic rules
//! on every entry written.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::*;
use crate::account::episode_map::{EpisodeMap, Identity, Segment, Shape};
use crate::account::pkce::Pkce;
use crate::account::provider::{
    CurrentEntry, MediaEvidence, ProviderKind, Tokens, TrackedMedia, UserListProvider, UserProfile,
};
use crate::account::status::ListStatus;
use crate::app::AppState;
use crate::cache::meta_cache_put;
use crate::commands::tracker_map::map_key;
use crate::error::AniError;

/// A tracker in memory: each entry's total and the user's row on it,
/// and every write it was sent, in order.
#[derive(Default)]
struct Tracker {
    totals: HashMap<u32, u32>,
    rows: Mutex<HashMap<u32, CurrentEntry>>,
    writes: Mutex<Vec<(u32, EntryUpdate)>>,
    reads: Mutex<Vec<u32>>,
}

impl Tracker {
    fn with(totals: &[(u32, u32)], rows: &[(u32, ListStatus, u32)]) -> Self {
        Self {
            totals: totals.iter().copied().collect(),
            rows: Mutex::new(
                rows.iter()
                    .map(|&(id, status, progress_episodes)| {
                        (
                            id,
                            CurrentEntry {
                                status,
                                progress_episodes,
                                times_rewatched: None,
                            },
                        )
                    })
                    .collect(),
            ),
            ..Default::default()
        }
    }

    fn writes(&self) -> Vec<(u32, Option<ListStatus>, Option<u32>)> {
        self.writes
            .lock()
            .expect("writes")
            .iter()
            .map(|(id, u)| (*id, u.status, u.progress_episodes))
            .collect()
    }

    fn reads(&self) -> Vec<u32> {
        self.reads.lock().expect("reads").clone()
    }
}

#[async_trait]
impl UserListProvider for Tracker {
    fn kind(&self) -> ProviderKind {
        ProviderKind::AniList
    }
    fn auth_url(&self, _pkce: &Pkce, _state: &str) -> crate::error::Result<String> {
        Err(AniError::Metadata)
    }
    async fn exchange_code(&self, _code: &str, _pkce: &Pkce) -> crate::error::Result<Tokens> {
        Err(AniError::Metadata)
    }
    async fn refresh(&self, _refresh_token: &str) -> crate::error::Result<Tokens> {
        Err(AniError::Metadata)
    }
    async fn me(&self, _tokens: &Tokens) -> crate::error::Result<UserProfile> {
        Ok(UserProfile {
            provider: ProviderKind::AniList,
            user_id: "u1".into(),
            username: "u".into(),
            avatar_url: None,
            stats: None,
        })
    }
    async fn list_all(&self, _tokens: &Tokens) -> crate::error::Result<Vec<ListEntry>> {
        Ok(Vec::new())
    }
    async fn update_entry(
        &self,
        _tokens: &Tokens,
        id: ProviderMediaId,
        update: EntryUpdate,
    ) -> crate::error::Result<ListEntry> {
        self.writes
            .lock()
            .expect("writes")
            .push((id.0, update.clone()));
        let mut rows = self.rows.lock().expect("rows");
        let row = rows.entry(id.0).or_insert(CurrentEntry {
            status: ListStatus::Planning,
            progress_episodes: 0,
            times_rewatched: None,
        });
        if let Some(status) = update.status {
            row.status = status;
        }
        if let Some(p) = update.progress_episodes {
            row.progress_episodes = p;
        }
        Ok(ListEntry {
            provider: ProviderKind::AniList,
            media_id: id,
            mal_id: None,
            status: row.status,
            progress_episodes: row.progress_episodes,
            score_0_to_100: None,
            updated_at_epoch_s: 1,
            title: String::new(),
        })
    }
    async fn delete_entry(
        &self,
        _tokens: &Tokens,
        _id: ProviderMediaId,
    ) -> crate::error::Result<()> {
        Err(AniError::Metadata)
    }
    async fn current_entry(
        &self,
        _tokens: &Tokens,
        id: ProviderMediaId,
    ) -> crate::error::Result<Option<CurrentEntry>> {
        Ok(self.rows.lock().expect("rows").get(&id.0).copied())
    }
    async fn tracked_media(
        &self,
        _tokens: &Tokens,
        id: ProviderMediaId,
    ) -> crate::error::Result<TrackedMedia> {
        self.reads.lock().expect("reads").push(id.0);
        Ok(TrackedMedia {
            entry: self.rows.lock().expect("rows").get(&id.0).copied(),
            episodes: self.totals.get(&id.0).copied(),
        })
    }
    async fn media_evidence(
        &self,
        _tokens: &Tokens,
        _id: ProviderMediaId,
    ) -> crate::error::Result<MediaEvidence> {
        // Every map here is seeded in the cache; building one is
        // tracker_map_test's business.
        Err(AniError::Metadata)
    }
}

fn state() -> Arc<AppState> {
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(
            reqwest::Client::new(),
            "http://unused-kitsu",
        ),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

/// The resolved entry and its total each recorded show writes through.
fn resolved_as(kitsu_id: &str) -> Option<(u32, Option<u32>)> {
    match kitsu_id {
        "186" => Some((210, Some(18))),
        "7882" => Some((19_603, Some(13))),
        "3579" => Some((4382, Some(28))),
        "1" => Some((21, Some(12))),
        _ => None,
    }
}

fn seed(state: &AppState, kitsu_id: &str, map: &EpisodeMap) {
    let body = serde_json::to_string(map).expect("serialise");
    meta_cache_put(
        &state.cache_pool,
        &map_key(ProviderKind::AniList, kitsu_id, resolved_as(kitsu_id)),
        &body,
        3600,
    )
    .expect("seed");
}

fn segment(entry: u32, total: u32, from: u32, to: u32, lead: u32, extras: &[u32]) -> Segment {
    Segment {
        entry,
        entry_total: total,
        entry_finished: true,
        kitsu_from: from,
        kitsu_to: to,
        lead,
        extras_after: extras.to_vec(),
    }
}

fn ranma_map() -> EpisodeMap {
    EpisodeMap {
        shape: Shape::Chain,
        segments: vec![
            segment(210, 18, 1, 18, 0, &[]),
            segment(149_939, 143, 19, 161, 0, &[]),
        ],
    }
}

fn watched(ep: u32) -> EntryUpdate {
    EntryUpdate {
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

fn finale(ep: u32) -> EntryUpdate {
    EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

async fn push(
    state: &Arc<AppState>,
    tracker: &Tracker,
    kitsu_id: &str,
    native: Option<u32>,
    update: EntryUpdate,
) -> Option<ListEntry> {
    let tokens = tokens();
    let ask = MapAsk {
        state,
        kind: ProviderKind::AniList,
        provider: tracker,
        tokens: &tokens,
        anilist_base: None,
    };
    push_progress_mapped(&ask, kitsu_id, native.map(ProviderMediaId), update)
        .await
        .expect("push ok")
}

use ListStatus::{Completed, Rewatching, Watching};

#[tokio::test]
async fn ranma_s_eighteenth_episode_completes_the_first_entry() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Watching, 17)]);
    let entry = push(&state, &tracker, "186", Some(210), watched(18)).await;
    assert_eq!(tracker.writes(), [(210, Some(Completed), Some(18))]);
    assert_eq!(entry.map(|e| e.media_id), Some(ProviderMediaId(210)));
}

#[tokio::test]
async fn ranma_s_nineteenth_episode_starts_nettou_hen_after_completing_the_first_entry() {
    let state = state();
    seed(&state, "186", &ranma_map());
    // The first entry is a few episodes short: it is brought to its
    // total and completed before Nettou-hen's first episode is written.
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Watching, 15)]);
    let entry = push(&state, &tracker, "186", Some(210), watched(19)).await;
    assert_eq!(
        tracker.writes(),
        [
            (210, Some(Completed), Some(18)),
            (149_939, Some(Watching), Some(1)),
        ]
    );
    assert_eq!(entry.map(|e| e.media_id), Some(ProviderMediaId(149_939)));
}

#[tokio::test]
async fn an_earlier_entry_already_complete_is_left_alone() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Completed, 18)]);
    push(&state, &tracker, "186", Some(210), watched(19)).await;
    assert_eq!(tracker.writes(), [(149_939, Some(Watching), Some(1))]);
}

#[tokio::test]
async fn an_earlier_entry_being_rewatched_is_brought_to_its_total_and_kept_rewatching() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Rewatching, 10)]);
    push(&state, &tracker, "186", Some(210), watched(19)).await;
    assert_eq!(
        tracker.writes(),
        [(210, None, Some(18)), (149_939, Some(Watching), Some(1))]
    );
}

#[tokio::test]
async fn a_segment_already_started_does_not_reread_the_entries_before_it() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(
        &[(210, 18), (149_939, 143)],
        &[(210, Completed, 18), (149_939, Watching, 40)],
    );
    push(&state, &tracker, "186", Some(210), watched(60)).await;
    assert_eq!(tracker.writes(), [(149_939, None, Some(42))]);
    // The resolved entry's read (the total the map is checked against)
    // and Nettou-hen's, and no read of 210 beyond that.
    assert_eq!(tracker.reads(), [210, 149_939]);
}

#[tokio::test]
async fn the_app_s_finale_completes_the_last_entry_at_its_own_total() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(
        &[(210, 18), (149_939, 143)],
        &[(210, Completed, 18), (149_939, Watching, 142)],
    );
    push(&state, &tracker, "186", Some(210), finale(161)).await;
    assert_eq!(tracker.writes(), [(149_939, Some(Completed), Some(143))]);
}

#[tokio::test]
async fn a_replayed_episode_never_lowers_an_entry() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(
        &[(210, 18), (149_939, 143)],
        &[(210, Completed, 18), (149_939, Watching, 90)],
    );
    // Kitsu's 30 is Nettou-hen's 12, below the row's 90: nothing is sent.
    assert!(push(&state, &tracker, "186", Some(210), watched(30))
        .await
        .is_none());
    assert!(tracker.writes().is_empty());
    // Kitsu's 10 is the first entry's, already complete at 18: nothing.
    assert!(push(&state, &tracker, "186", Some(210), watched(10))
        .await
        .is_none());
    assert!(tracker.writes().is_empty());
}

#[tokio::test]
async fn the_last_episode_of_a_segment_being_rewatched_keeps_the_rewatch() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Rewatching, 17)]);
    push(&state, &tracker, "186", Some(210), watched(18)).await;
    assert_eq!(tracker.writes(), [(210, None, Some(18))]);
}

#[tokio::test]
async fn a_prologue_the_tracker_counts_shifts_the_number_and_the_finale_completes() {
    let state = state();
    seed(
        &state,
        "7882",
        &EpisodeMap {
            shape: Shape::Extras,
            segments: vec![segment(19_603, 13, 1, 12, 0, &[0])],
        },
    );
    let tracker = Tracker::with(&[(19_603, 13)], &[]);
    push(&state, &tracker, "7882", Some(19_603), watched(1)).await;
    push(&state, &tracker, "7882", Some(19_603), finale(12)).await;
    assert_eq!(
        tracker.writes(),
        [
            (19_603, Some(Watching), Some(2)),
            (19_603, Some(Completed), Some(13)),
        ]
    );
}

#[tokio::test]
async fn a_surplus_nothing_explains_sends_kitsu_s_number_and_never_completes() {
    // Haruhi 2009 on AniList: 28 against Kitsu's 14, the map identity.
    let state = state();
    seed(&state, "3579", &EpisodeMap::identity(Identity::Unexplained));
    let tracker = Tracker::with(&[(4382, 28)], &[(4382, Watching, 13)]);
    push(&state, &tracker, "3579", Some(4382), finale(14)).await;
    assert_eq!(tracker.writes(), [(4382, None, Some(14))]);
}

#[tokio::test]
async fn agreeing_counts_send_the_episode_as_they_always_have() {
    let state = state();
    seed(&state, "1", &EpisodeMap::identity(Identity::CountsAgree));
    let tracker = Tracker::with(&[(21, 12)], &[(21, Watching, 11)]);
    push(&state, &tracker, "1", Some(21), finale(12)).await;
    assert_eq!(tracker.writes(), [(21, Some(Completed), Some(12))]);
}

#[tokio::test]
async fn a_later_part_with_no_entry_of_its_own_writes_its_prequel_s_entry() {
    // Tower of God's Koubou-sen: no AniList entry; filed under season
    // 2's 153406 after its 13 episodes.
    let state = state();
    seed(
        &state,
        "49244",
        &EpisodeMap {
            shape: Shape::SplitParts,
            segments: vec![segment(153_406, 26, 1, 13, 13, &[])],
        },
    );
    let tracker = Tracker::with(&[(153_406, 26)], &[(153_406, Watching, 13)]);
    let entry = push(&state, &tracker, "49244", None, watched(1)).await;
    assert_eq!(tracker.writes(), [(153_406, None, Some(14))]);
    assert_eq!(entry.map(|e| e.progress_episodes), Some(14));
    push(&state, &tracker, "49244", None, finale(13)).await;
    assert_eq!(tracker.writes()[1], (153_406, Some(Completed), Some(26)));
}

/// A show Kitsu resolves to nothing, filed at the first episode of an
/// entry: an entry the user has finished, or is past that episode on,
/// or is rewatching beyond it, is sent nothing, so no rewatch is ever
/// started; an entry not on the list is added at that episode.
#[tokio::test]
async fn a_mapped_write_below_an_entry_s_progress_starts_no_rewatch() {
    let state = state();
    seed(
        &state,
        "9718",
        &EpisodeMap {
            shape: Shape::SpecialExtras,
            segments: vec![segment(19_603, 13, 1, 1, 0, &[])],
        },
    );
    for row in [(Completed, 13), (Watching, 5), (Rewatching, 3)] {
        let tracker = Tracker::with(&[(19_603, 13)], &[(19_603, row.0, row.1)]);
        assert!(
            push(&state, &tracker, "9718", None, watched(1))
                .await
                .is_none(),
            "{row:?}"
        );
        assert!(
            tracker.writes().is_empty(),
            "{row:?}: {:?}",
            tracker.writes()
        );
    }
    let tracker = Tracker::with(&[(19_603, 13)], &[]);
    push(&state, &tracker, "9718", None, finale(1)).await;
    assert_eq!(tracker.writes(), [(19_603, Some(Watching), Some(1))]);
}

#[tokio::test]
async fn a_show_that_resolves_to_nothing_and_maps_to_nothing_writes_nothing() {
    let state = state();
    seed(&state, "5", &EpisodeMap::identity(Identity::Unexplained));
    let tracker = Tracker::with(&[], &[]);
    assert!(push(&state, &tracker, "5", None, watched(1))
        .await
        .is_none());
    assert!(tracker.writes().is_empty());
}

#[tokio::test]
async fn an_episode_past_the_map_falls_back_to_identity_clamped() {
    let state = state();
    seed(&state, "186", &ranma_map());
    // Kitsu's 162 (a provider extra listed past Kitsu's count): no
    // segment; identity on the resolved entry, clamped to its 18, which
    // the row already has.
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Completed, 18)]);
    assert!(push(&state, &tracker, "186", Some(210), watched(162))
        .await
        .is_none());
    assert!(tracker.writes().is_empty());
}

#[tokio::test]
async fn a_status_only_write_goes_to_the_resolved_entry_as_it_did() {
    let state = state();
    seed(&state, "186", &ranma_map());
    let tracker = Tracker::with(&[(210, 18)], &[]);
    let update = EntryUpdate {
        status: Some(ListStatus::Paused),
        ..Default::default()
    };
    push(&state, &tracker, "186", Some(210), update).await;
    assert_eq!(tracker.writes(), [(210, Some(ListStatus::Paused), None)]);
}

/// Runs `push` while the entry lock for `entry` is held, releasing it
/// once the push has queued behind it, and answers how many writes the
/// tracker had received by the release, and the push's result. Queued,
/// not merely not there yet: a push that has not reached the lock has
/// not written past it either, and would prove nothing.
async fn push_behind_entry_lock(
    state: &Arc<AppState>,
    tracker: &Tracker,
    entry: u32,
    kitsu_id: &str,
    native: Option<u32>,
    update: EntryUpdate,
) -> (usize, Option<ListEntry>) {
    let lock = state
        .account_write_locks
        .for_entry(ProviderKind::AniList, ProviderMediaId(entry));
    let guard = lock.lock().await;
    let queue = crate::held::queue(crate::held::lock_key(&*lock));
    let pushed = push(state, tracker, kitsu_id, native, update);
    tokio::pin!(pushed);
    tokio::select! {
        () = queue.joined() => {}
        _ = &mut pushed => panic!("the push finished without waiting for the entry's lock"),
    }
    let before = tracker.writes().len();
    drop(guard);
    (before, pushed.await)
}

#[tokio::test]
async fn a_part_s_write_waits_for_another_kitsu_entry_s_write_to_the_same_tracker_entry() {
    // Koubou-sen (49244) and season 2 (46312) are both filed under
    // AniList's 153406, each Kitsu entry with its own show lock: the
    // entry's own lock is what keeps one's read-then-write from
    // landing between the other's.
    let state = state();
    seed(
        &state,
        "49244",
        &EpisodeMap {
            shape: Shape::SplitParts,
            segments: vec![segment(153_406, 26, 1, 13, 13, &[])],
        },
    );
    let tracker = Tracker::with(&[(153_406, 26)], &[(153_406, Watching, 13)]);
    let (before, got) =
        push_behind_entry_lock(&state, &tracker, 153_406, "49244", None, watched(1)).await;
    assert_eq!(before, 0, "the write waited for the entry's lock");
    assert_eq!(got.map(|e| e.progress_episodes), Some(14));
}

#[tokio::test]
async fn an_identity_write_waits_for_its_entry_s_lock() {
    let state = state();
    seed(&state, "1", &EpisodeMap::identity(Identity::CountsAgree));
    let tracker = Tracker::with(&[(21, 12)], &[(21, Watching, 3)]);
    let (before, got) =
        push_behind_entry_lock(&state, &tracker, 21, "1", Some(21), watched(4)).await;
    assert_eq!(before, 0);
    assert!(got.is_some());
    // The status-only write too.
    let update = EntryUpdate {
        status: Some(ListStatus::Paused),
        ..Default::default()
    };
    let (before, got) = push_behind_entry_lock(&state, &tracker, 21, "1", Some(21), update).await;
    assert_eq!(before, 1);
    assert!(got.is_some());
}

#[tokio::test]
async fn an_earlier_segment_s_write_waits_for_its_entry_s_lock() {
    // Ranma's map under a Kitsu id that resolves to nothing, so the
    // write takes no lock on 210 before its segment's entry: the first
    // wait on 210 is the earlier segment being brought to its total.
    let state = state();
    seed(&state, "18600", &ranma_map());
    let tracker = Tracker::with(&[(210, 18), (149_939, 143)], &[(210, Watching, 15)]);
    let (before, _) =
        push_behind_entry_lock(&state, &tracker, 210, "18600", None, watched(19)).await;
    assert_eq!(before, 0);
    assert_eq!(tracker.writes().len(), 2);
}

#[test]
fn an_entry_s_lock_is_one_per_tracker_entry_and_apart_from_the_show_locks() {
    let locks = crate::commands::account::AccountWriteLocks::new();
    let a = locks.for_entry(ProviderKind::AniList, ProviderMediaId(7));
    let b = locks.for_entry(ProviderKind::AniList, ProviderMediaId(7));
    assert!(Arc::ptr_eq(&a, &b));
    assert!(!Arc::ptr_eq(
        &a,
        &locks.for_entry(ProviderKind::AniList, ProviderMediaId(8))
    ));
    assert!(!Arc::ptr_eq(
        &a,
        &locks.for_entry(ProviderKind::MyAnimeList, ProviderMediaId(7))
    ));
    assert!(!Arc::ptr_eq(
        &a,
        &locks.for_show(ProviderKind::AniList, "7")
    ));
}
