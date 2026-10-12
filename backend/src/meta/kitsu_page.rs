//! One page of a Kitsu anime text search.
//!
//! Kitsu's anime search ranks by popularity and caps `page[limit]` at
//! 20, so a broad query buries new or minor shows several pages deep.
//! A page carries where the next one starts, so the caller can walk
//! on without knowing the page size.

use serde::{Deserialize, Serialize};

use super::kitsu::KitsuAnimeRef;

/// A search page: its hits, and the raw offset of the following page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitsuSearchPage {
    /// The page's hits, music videos already dropped.
    pub hits: Vec<KitsuAnimeRef>,
    /// `page[offset]` of the next page, `None` on the last one. Counted
    /// in Kitsu's own rows, before any hit was dropped here.
    pub next_offset: Option<u32>,
}

/// Where the page after `offset` starts, or `None` when this one was
/// the last.
///
/// `returned` is how many rows Kitsu sent for this page (before any
/// were dropped), `has_next_link` whether it sent `links.next`, and
/// `count` its `meta.count`.
#[must_use]
pub fn next_offset(
    offset: u32,
    limit: u8,
    returned: usize,
    has_next_link: bool,
    count: Option<u64>,
) -> Option<u32> {
    if returned == 0 {
        return None;
    }
    let next = offset.checked_add(u32::from(limit))?;
    let counted_more = count.is_some_and(|c| c > u64::from(next));
    (has_next_link || counted_more).then_some(next)
}

#[cfg(test)]
#[path = "kitsu_page_test.rs"]
mod tests;
