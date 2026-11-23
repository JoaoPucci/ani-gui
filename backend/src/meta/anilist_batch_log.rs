//! The line a lane logs for each request it sends (stub).

use std::future::Future;

use crate::error::Result;

/// Which request a lane sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sent {
    /// One show's single lookup.
    Single,
    /// A `Page` of MAL ids.
    MalPage,
    /// A `Page` of AniList ids.
    AniListIdPage,
}

/// Stub: `request`, unlogged.
pub(crate) async fn logged<T, F: Future<Output = Result<T>>>(
    _kind: &'static str,
    _sent: Sent,
    _ids: usize,
    request: F,
) -> Result<T> {
    request.await
}

#[cfg(test)]
#[path = "anilist_batch_log_test.rs"]
mod tests;
