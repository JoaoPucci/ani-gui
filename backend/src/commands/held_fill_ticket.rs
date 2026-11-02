//! A ticket's life ([`super`]): the pages watching it, the release of
//! one no page shows, the end of its retry, and the turn retries take
//! one at a time. Split from [`super`] so each file stays inside the
//! CRAP gate's per-file bar.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use super::{registry, Fill, ENDED};

/// Ends a retry's ticket when the retry is done with it — returned,
/// or dropped unfinished (a panic, the runtime going down), which ends
/// it unlanded rather than leaving the show's key taken for good.
pub(super) struct Ending {
    pub(super) key: String,
    pub(super) ticket: u64,
    pub(super) fill: Fill,
    pub(super) kitsu_id: String,
    pub(super) landed: bool,
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
            fill = ?self.fill,
            kitsu_id = %self.kitsu_id,
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

/// The ticket a fill pending under `key` waits on, if one does. When a
/// page's read is being served ([`super::collect`]), the page is about
/// to wait on it: it counts as on screen again, after a release too.
pub(super) fn serve_pending(key: &str) -> Option<u64> {
    let mut registry = registry();
    let ticket = registry.pending(key)?;
    if super::NOTED.try_with(|_| ()).is_ok() {
        registry.served(ticket, Instant::now());
    }
    Some(ticket)
}

/// While alive, a page waits on its tickets: they count as on screen
/// until it ends or a release lets them go.
pub(super) struct Watching(Vec<(u64, u64)>);

impl Watching {
    pub(super) fn new(tickets: &[u64]) -> Self {
        let now = Instant::now();
        let mut registry = registry();
        Self(
            tickets
                .iter()
                .map(|&t| (t, registry.watch(t, now)))
                .collect(),
        )
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        let now = Instant::now();
        let mut registry = registry();
        for &(t, era) in &self.0 {
            registry.unwatch(t, era, now);
        }
    }
}

/// One retry at a time per AniList endpoint.
pub(super) fn turn(url: &str) -> Arc<tokio::sync::Mutex<()>> {
    static TURNS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
        LazyLock::new(Default::default);
    TURNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(url.to_owned())
        .or_default()
        .clone()
}
