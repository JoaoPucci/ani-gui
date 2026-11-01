//! The cached list against a tracker held in memory, under any
//! interleaving of full-list refreshes, the app's own writes, and
//! changes made on the tracker's website.
//!
//! Every answer the app receives about a row is timed: a write's answer
//! when the write is sent, a list read's when the read is sent. Once
//! every refresh has finished, each cached row must be the latest answer
//! received for it — a refresh may not put back a row a newer write
//! changed, and a write landing during a refresh may not cost the
//! changes that refresh read for the other rows. And a refresh reads the
//! list once, however many writes land while it is out.
//!
//! A removal's live read and its local clear of the cached row run too.
//! A live read is an answer the app keeps only through the clear that
//! follows it: a read that found the row absent, when the clear for it
//! runs, counts as an answer of absence at the read's time.
//!
//! The model sends a refresh's list read the moment the refresh starts.
//! In the app the read goes after the owner lookup, and a paginated list
//! is read page by page, so a write can land after the refresh started
//! but before the page carrying its row was read. The refresh then keeps
//! the write's answer for that row even if the page saw a later website
//! change to it: the cached row can be one answer behind, never ahead of
//! the write, until the next refresh replaces it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use proptest::prelude::*;
use tokio::sync::{mpsc, oneshot};

use crate::account::pkce::Pkce;
use crate::account::provider::{
    CurrentEntry, EntryUpdate, ListEntry, MediaEvidence, ProviderKind, ProviderMediaId, Tokens,
    TrackedMedia, UserListProvider, UserProfile,
};
use crate::account::status::ListStatus;
use crate::app::AppState;
use crate::error::{AniError, Result};

const KIND: ProviderKind = ProviderKind::AniList;
const USER: &str = "u1";
/// The rows a run touches.
const ROWS: [u32; 3] = [1, 2, 3];
const STATUSES: [ListStatus; 4] = [
    ListStatus::Planning,
    ListStatus::Watching,
    ListStatus::Completed,
    ListStatus::Paused,
];

/// What happened while a run was out, for the test to wait on.
enum Event {
    /// A list read was sent and is waiting to be answered.
    Asked,
    /// A refresh finished, one way or the other.
    Done,
}

/// The tracker: its rows, a clock every answer is stamped from, and the
/// list reads waiting for the test to answer them.
struct Tracker {
    rows: Mutex<HashMap<u32, ListStatus>>,
    clock: Arc<AtomicU64>,
    /// Each list read sent: when, and what it answered.
    reads: Mutex<Vec<(u64, HashMap<u32, ListStatus>)>>,
    waiting: Mutex<Vec<oneshot::Sender<()>>>,
    events: mpsc::UnboundedSender<Event>,
}

fn entry(id: u32, status: ListStatus) -> ListEntry {
    ListEntry {
        provider: KIND,
        media_id: ProviderMediaId(id),
        mal_id: None,
        status,
        progress_episodes: 0,
        score_0_to_100: None,
        updated_at_epoch_s: 1,
        title: String::new(),
    }
}

#[async_trait]
impl UserListProvider for Tracker {
    fn kind(&self) -> ProviderKind {
        KIND
    }
    fn auth_url(&self, _pkce: &Pkce, _state: &str) -> Result<String> {
        Err(AniError::Metadata)
    }
    async fn exchange_code(&self, _code: &str, _pkce: &Pkce) -> Result<Tokens> {
        Err(AniError::Metadata)
    }
    async fn refresh(&self, _refresh_token: &str) -> Result<Tokens> {
        Err(AniError::Metadata)
    }
    async fn me(&self, _tokens: &Tokens) -> Result<UserProfile> {
        Ok(UserProfile {
            provider: KIND,
            user_id: USER.into(),
            username: "u".into(),
            avatar_url: None,
            stats: None,
        })
    }
    async fn list_all(&self, _tokens: &Tokens) -> Result<Vec<ListEntry>> {
        // The tracker answers as it stands when the read is sent; the
        // answer reaches the app when the test lets it.
        let snapshot = self.rows.lock().expect("rows").clone();
        let at = self.clock.fetch_add(1, Ordering::SeqCst);
        self.reads
            .lock()
            .expect("reads")
            .push((at, snapshot.clone()));
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().expect("waiting").push(tx);
        let _ = self.events.send(Event::Asked);
        let _ = rx.await;
        Ok(snapshot.into_iter().map(|(id, s)| entry(id, s)).collect())
    }
    async fn update_entry(
        &self,
        _tokens: &Tokens,
        id: ProviderMediaId,
        update: EntryUpdate,
    ) -> Result<ListEntry> {
        let status = update.status.expect("every write here sets a status");
        self.rows.lock().expect("rows").insert(id.0, status);
        Ok(entry(id.0, status))
    }
    async fn delete_entry(&self, _tokens: &Tokens, _id: ProviderMediaId) -> Result<()> {
        Err(AniError::Metadata)
    }
    async fn current_entry(
        &self,
        _tokens: &Tokens,
        id: ProviderMediaId,
    ) -> Result<Option<CurrentEntry>> {
        Ok(self
            .rows
            .lock()
            .expect("rows")
            .get(&id.0)
            .map(|&status| CurrentEntry {
                status,
                progress_episodes: 0,
                times_rewatched: None,
            }))
    }
    async fn tracked_media(&self, _tokens: &Tokens, _id: ProviderMediaId) -> Result<TrackedMedia> {
        Err(AniError::Metadata)
    }
    async fn media_evidence(
        &self,
        _tokens: &Tokens,
        _id: ProviderMediaId,
    ) -> Result<MediaEvidence> {
        Err(AniError::Metadata)
    }
}

/// One step of a run.
#[derive(Debug, Clone)]
enum Step {
    /// A full-list refresh starts and sends its list read.
    Refresh,
    /// The app writes a status to a row (an editor save or a watched
    /// episode), through to the tracker and the cache.
    Write(usize, usize),
    /// The row changes on the tracker's website: a status, or removed.
    Website(usize, Option<usize>),
    /// The waiting list read at this index (modulo those waiting) is
    /// answered.
    Answer(usize),
    /// A removal reads the row live from the tracker.
    Read(usize),
    /// A removal clears the row from the cache, by its last live read.
    Clear(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        Just(Step::Refresh),
        (0..ROWS.len(), 0..STATUSES.len()).prop_map(|(r, s)| Step::Write(r, s)),
        (0..ROWS.len(), proptest::option::of(0..STATUSES.len()))
            .prop_map(|(r, s)| Step::Website(r, s)),
        any::<usize>().prop_map(Step::Answer),
        (0..ROWS.len()).prop_map(Step::Read),
        (0..ROWS.len()).prop_map(Step::Clear),
    ]
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
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), "http://unused"),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// What a run left: the cache, the latest answer the app received for
/// each row, how many refreshes started and how many list reads went.
struct Outcome {
    cached: HashMap<u32, ListStatus>,
    latest: HashMap<u32, Option<ListStatus>>,
    refreshes: usize,
    reads: usize,
}

async fn run(steps: Vec<Step>) -> Outcome {
    let state = state();
    let (events, mut heard) = mpsc::unbounded_channel();
    let clock = Arc::new(AtomicU64::new(0));
    let tracker = Arc::new(Tracker {
        rows: Mutex::new(HashMap::new()),
        clock: clock.clone(),
        reads: Mutex::new(Vec::new()),
        waiting: Mutex::new(Vec::new()),
        events: events.clone(),
    });
    // Each write's answer: when it was sent, which row, what it said.
    let mut written: Vec<(u64, u32, ListStatus)> = Vec::new();
    // Each row's last live read: when, and what it found.
    let mut last_read: HashMap<u32, (u64, Option<ListStatus>)> = HashMap::new();
    // Absences the app acted on: a clear after a read that found none.
    let mut cleared: Vec<(u64, u32)> = Vec::new();
    let mut running = 0usize;
    let mut refreshes = 0usize;
    // Answer a waiting read and wait for what that leads to: another
    // read from the same refresh, or the refresh finishing.
    async fn answer(
        tracker: &Tracker,
        heard: &mut mpsc::UnboundedReceiver<Event>,
        running: &mut usize,
        at: usize,
    ) {
        let tx = {
            let mut waiting = tracker.waiting.lock().expect("waiting");
            if waiting.is_empty() {
                return;
            }
            let i = at % waiting.len();
            waiting.remove(i)
        };
        let _ = tx.send(());
        match heard.recv().await.expect("the refresh reports") {
            Event::Asked => {}
            Event::Done => *running -= 1,
        }
    }
    for step in steps {
        match step {
            Step::Refresh => {
                refreshes += 1;
                running += 1;
                let (state, tracker, events) = (state.clone(), tracker.clone(), events.clone());
                tokio::spawn(async move {
                    let _ = super::list_all_and_cache_via(
                        &state,
                        KIND,
                        tracker.as_ref(),
                        &tokens(),
                        USER,
                    )
                    .await;
                    let _ = events.send(Event::Done);
                });
                match heard.recv().await.expect("the refresh reports") {
                    Event::Asked => {}
                    Event::Done => running -= 1,
                }
            }
            Step::Write(r, s) => {
                let (id, status) = (ROWS[r], STATUSES[s]);
                let at = clock.fetch_add(1, Ordering::SeqCst);
                super::write_through(
                    &state,
                    KIND,
                    tracker.as_ref(),
                    &tokens(),
                    ProviderMediaId(id),
                    EntryUpdate {
                        status: Some(status),
                        ..Default::default()
                    },
                    state.account_write_locks.epochs.current(KIND),
                )
                .await
                .expect("the write lands");
                written.push((at, id, status));
            }
            Step::Website(r, s) => {
                clock.fetch_add(1, Ordering::SeqCst);
                let mut rows = tracker.rows.lock().expect("rows");
                match s {
                    Some(s) => rows.insert(ROWS[r], STATUSES[s]),
                    None => rows.remove(&ROWS[r]),
                };
            }
            Step::Answer(at) => answer(&tracker, &mut heard, &mut running, at).await,
            Step::Read(r) => {
                let id = ROWS[r];
                let at = clock.fetch_add(1, Ordering::SeqCst);
                let found = crate::commands::account_edit::read_entry_at(
                    &state,
                    KIND,
                    tracker.as_ref(),
                    &tokens(),
                    ProviderMediaId(id),
                )
                .await
                .expect("the read answers")
                .map(|e| e.status);
                last_read.insert(id, (at, found));
            }
            Step::Clear(r) => {
                let id = ROWS[r];
                crate::commands::account_edit::clear_cached_row(
                    &state,
                    KIND,
                    state.account_write_locks.epochs.current(KIND),
                    id,
                    USER,
                );
                if let Some(&(at, None)) = last_read.get(&id) {
                    cleared.push((at, id));
                }
            }
        }
    }
    while running > 0 {
        answer(&tracker, &mut heard, &mut running, 0).await;
    }
    // The latest answer per row, from writes and list reads alike.
    let mut latest: HashMap<u32, (u64, Option<ListStatus>)> = HashMap::new();
    let mut hear = |id: u32, at: u64, said: Option<ListStatus>| {
        let slot = latest.entry(id).or_insert((0, None));
        if at >= slot.0 {
            *slot = (at, said);
        }
    };
    for &(at, id, status) in &written {
        hear(id, at, Some(status));
    }
    for &(at, id) in &cleared {
        hear(id, at, None);
    }
    let reads = tracker.reads.lock().expect("reads").clone();
    for (at, snapshot) in &reads {
        for id in ROWS {
            hear(id, *at, snapshot.get(&id).copied());
        }
    }
    let cached = crate::account::cache::list_entries(&state.cache_pool, KIND, USER)
        .expect("cache read")
        .into_iter()
        .map(|e| (e.media_id.0, e.status))
        .collect();
    Outcome {
        cached,
        latest: latest.into_iter().map(|(id, (_, s))| (id, s)).collect(),
        refreshes,
        reads: reads.len(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn each_cached_row_is_the_latest_answer_and_each_refresh_reads_once(
        steps in proptest::collection::vec(step(), 1..24)
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let out = rt.block_on(run(steps));
        for id in ROWS {
            let latest = out.latest.get(&id).copied().flatten();
            prop_assert_eq!(
                out.cached.get(&id).copied(),
                latest,
                "row {} is cached as {:?}, but the latest answer was {:?}",
                id,
                out.cached.get(&id),
                latest
            );
        }
        prop_assert_eq!(out.reads, out.refreshes, "a refresh read the list more than once");
    }
}
