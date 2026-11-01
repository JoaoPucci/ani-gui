//! Background fills the AniList gate held back, retried once the pause
//! ends.
//!
//! The gate ([`crate::meta::anilist_gate`]) holds a background fill back
//! rather than let AniList answer 429, and a fill held back caches
//! nothing — an unanswered request is not an absence. Before this, the
//! fill was asked again only when a page asked for it again, so a show's
//! episode thumbnails or banner stayed empty until the user left and
//! came back. Now a fill held back is queued for a retry:
//!
//! - **Once per pause.** The retry waits until the gate would let a
//!   retry through, runs the fill again, and is done when the fill is
//!   in the cache. Held back again, it waits out that pause too, up to
//!   [`MAX_RETRIES`] pauses; any other failure ends it. A fill only
//!   ever caches a real answer, so a retry caches nothing the first
//!   attempt would not have.
//! - **Deduped by item.** A fill held back while its retry is pending
//!   joins that retry and its ticket.
//! - **Behind the user.** Retries run one at a time, in a class of
//!   their own ([`crate::meta::anilist_gate::Class::Retry`]): none takes
//!   a turn while any other request waits for one, and only while the
//!   window holds fewer than
//!   [`crate::meta::anilist_gate::Pace::retry_capacity`] requests — half
//!   the background share — so they never touch the reserve held for
//!   the user, leave the rest of the background share to the page on
//!   screen, and do not refill the window the pause just emptied.
//! - **On screen first.** A fill counts as on screen while a page waits
//!   on its ticket ([`wait_any`]). A page that closes lets its tickets go
//!   ([`release`]); their retries still go — one answer per show is
//!   cached for a day, so it serves the next view — but only when no
//!   fill on screen waits for a retry.
//! - **Bounded.** At most [`registry::MAX_PENDING`] fills wait at once;
//!   one held back past that gets no retry, as before this existed. Only
//!   a read served to a page is retried: one no page asked for is asked
//!   again by its next caller, as before.
//!
//! The page that was served without the fill learns of the retry
//! through a ticket: the routes attach the tickets a request was held
//! under ([`collect`]), and the page waits on them ([`wait_any`]) to
//! fetch once more when its fill lands.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::app::AppState;
use crate::error::AniError;

#[path = "held_fill_job.rs"]
mod job;
#[path = "held_fill_registry.rs"]
mod registry;
use registry::{Registry, Standing};

/// Which fill was held back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fill {
    /// A show's episode thumbnails from AniList's streaming listing.
    Thumbs,
    /// A show's detail row, held for its AniList banner.
    Detail,
}

tokio::task_local! {
    /// The tickets the request being served was held under.
    static NOTED: RefCell<Vec<u64>>;
    /// Set while a retry runs: its own fill held back again is the
    /// retry's to schedule, not a new item, and is recorded here.
    static RETRYING: Cell<bool>;
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Default::default);

/// Bumped whenever a retry ends, for [`wait_any`].
static ENDED: LazyLock<tokio::sync::watch::Sender<u64>> =
    LazyLock::new(|| tokio::sync::watch::channel(0).0);

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Run `f`, and return with its output the tickets of every fill it was
/// served without that a retry will bring.
pub(crate) async fn collect<F: Future>(f: F) -> (F::Output, Vec<u64>) {
    NOTED
        .scope(RefCell::new(Vec::new()), async {
            let out = f.await;
            let noted = NOTED.with(RefCell::take);
            (out, noted)
        })
        .await
}

fn note(ticket: u64) {
    let _ = NOTED.try_with(|noted| {
        let mut noted = noted.borrow_mut();
        if !noted.contains(&ticket) {
            noted.push(ticket);
        }
    });
}

/// The key a fill is deduped under: per app state (two tests' states
/// never share), AniList endpoint, fill and show.
fn key(state: &AppState, fill: Fill, kitsu_id: &str, anilist_base: Option<&str>) -> String {
    format!(
        "{:p}\u{0}{}\u{0}{fill:?}\u{0}{kitsu_id}",
        std::ptr::from_ref(state),
        anilist_base.unwrap_or("")
    )
}

fn url(anilist_base: Option<&str>) -> &str {
    anilist_base.unwrap_or(crate::meta::anilist::ANILIST_API)
}

/// The request being served joins `fill`'s pending retry, if it has
/// one — a caller that took a copy of another's held-back answer.
pub(crate) fn note_pending(
    state: &AppState,
    fill: Fill,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) {
    if let Some(ticket) = registry().pending(&key(state, fill, kitsu_id, anilist_base)) {
        note(ticket);
    }
}

/// `fill` got `err` instead of an answer. When that is the gate holding
/// it back, or AniList refusing this client, and the gate is now in a
/// pause, the fill is queued for a retry once the pause ends — or joins
/// the retry already queued — and the request being served notes its
/// ticket. Any other failure is not retried here: the next view asks
/// again.
pub(crate) fn on_unanswered(
    state: &AppState,
    fill: Fill,
    kitsu_id: &str,
    anilist_base: Option<&str>,
    err: &AniError,
) {
    let refused = matches!(
        err,
        AniError::RateLimited {
            refused_by: Some("anilist"),
            ..
        }
    );
    // A retry's own run tells its retry whether it was held back again.
    let retrying = RETRYING.try_with(|held| held.set(held.get() || refused));
    if !refused || retrying.is_ok() {
        return;
    }
    // Only a read served to a page is retried: the page is told, and
    // waits. A read no page asked for — an import, a guard reading a
    // show's detail — is asked again by its next caller, as before.
    if NOTED.try_with(|_| ()).is_err() {
        return;
    }
    if crate::meta::anilist_gate::background_wait(url(anilist_base)).is_zero() {
        return;
    }
    let key = key(state, fill, kitsu_id, anilist_base);
    let Some((ticket, fresh)) = registry().hold(&key, Instant::now()) else {
        tracing::info!(
            ?fill,
            kitsu_id,
            "held-back fill not queued: the retry queue is full"
        );
        return;
    };
    note(ticket);
    if fresh {
        let job = job::Job {
            state: state.clone(),
            fill,
            kitsu_id: kitsu_id.to_owned(),
            anilist_base: anilist_base.map(str::to_owned),
            ticket,
        };
        tracing::info!(?fill, kitsu_id, ticket, "held-back fill queued for a retry");
        tokio::spawn(async move {
            let mut ending = Ending {
                key,
                ticket,
                landed: false,
            };
            ending.landed = job.retry().await;
        });
    }
}

/// Ends a retry's ticket when the retry is done with it — returned,
/// or dropped unfinished (a panic, the runtime going down), which ends
/// it unlanded rather than leaving the show's key taken for good.
struct Ending {
    key: String,
    ticket: u64,
    landed: bool,
}

impl Drop for Ending {
    fn drop(&mut self) {
        let on_screen = {
            let mut registry = registry();
            let on_screen = registry.on_screen(self.ticket, Instant::now());
            registry.end(&self.key, self.ticket, self.landed);
            on_screen
        };
        ENDED.send_modify(|n| *n += 1);
        tracing::info!(
            ticket = self.ticket,
            landed = self.landed,
            on_screen,
            "held-back fill retry ended"
        );
    }
}

/// The pages showing these tickets' fills have gone. Their retries
/// still go — a landed fill is cached for the next view — but only once
/// no fill on screen waits for one.
pub(crate) fn release(tickets: &[u64]) {
    let mut registry = registry();
    for &t in tickets {
        registry.release(t);
    }
}

/// While alive, a page waits on its tickets: they count as on screen.
struct Watching<'a>(&'a [u64]);

impl<'a> Watching<'a> {
    fn new(tickets: &'a [u64]) -> Self {
        let now = Instant::now();
        let mut registry = registry();
        for &t in tickets {
            registry.watch(t, now);
        }
        Self(tickets)
    }
}

impl Drop for Watching<'_> {
    fn drop(&mut self) {
        let now = Instant::now();
        let mut registry = registry();
        for &t in self.0 {
            registry.unwatch(t, now);
        }
    }
}

/// One retry at a time per AniList endpoint.
fn turn(url: &str) -> Arc<tokio::sync::Mutex<()>> {
    static TURNS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
        LazyLock::new(Default::default);
    TURNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(url.to_owned())
        .or_default()
        .clone()
}

/// Wait until one of `tickets` has ended, at most `limit`, and return
/// the ended ones with whether each landed. A ticket this process never
/// issued has ended unlanded. Empty when `limit` passed first.
pub(crate) async fn wait_any(tickets: &[u64], limit: Duration) -> Vec<(u64, bool)> {
    let deadline = tokio::time::Instant::now() + limit;
    let mut ended = ENDED.subscribe();
    let _watching = Watching::new(tickets);
    loop {
        let done: Vec<(u64, bool)> = {
            let registry = registry();
            tickets
                .iter()
                .filter_map(|&t| match registry.standing(t) {
                    Standing::Pending => None,
                    Standing::Ended(landed) => Some((t, landed)),
                })
                .collect()
        };
        if !done.is_empty() || tickets.is_empty() {
            return done;
        }
        if tokio::time::timeout_at(deadline, ended.changed())
            .await
            .is_err()
        {
            return Vec::new();
        }
    }
}

#[cfg(test)]
#[path = "held_fill_registry_test.rs"]
mod registry_tests;

#[cfg(test)]
#[path = "held_fill_test.rs"]
mod tests;
