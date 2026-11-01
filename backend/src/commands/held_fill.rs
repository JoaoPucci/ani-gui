//! Background fills the AniList gate held back, and the tickets a page
//! waits on for their retry.
//!
//! Nothing is retried yet: a fill held back is not queued, no read
//! carries a ticket, and a wait answers every ticket as ended unlanded.
//! The routes and the fills call through here so the retry can land in
//! one place.

use std::cell::RefCell;
use std::future::Future;
use std::time::Duration;

use crate::app::AppState;
use crate::error::AniError;

#[path = "held_fill_registry.rs"]
mod registry;

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

/// The request being served joins `fill`'s pending retry, if it has
/// one. None has yet.
pub(crate) fn note_pending(
    _state: &AppState,
    _fill: Fill,
    _kitsu_id: &str,
    _anilist_base: Option<&str>,
) {
}

/// `fill` got `err` instead of an answer. Not retried yet: the next
/// view asks again.
pub(crate) fn on_unanswered(
    _state: &AppState,
    _fill: Fill,
    _kitsu_id: &str,
    _anilist_base: Option<&str>,
    _err: &AniError,
) {
}

/// The pages showing these tickets' fills have gone.
pub(crate) fn release(_tickets: &[u64]) {}

/// Every ticket has ended unlanded: none is ever issued yet.
pub(crate) async fn wait_any(tickets: &[u64], _limit: Duration) -> Vec<(u64, bool)> {
    tickets.iter().map(|&t| (t, false)).collect()
}
