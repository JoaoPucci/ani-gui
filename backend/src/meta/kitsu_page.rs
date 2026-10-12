//! One page of a Kitsu anime text search.
//!
//! Kitsu's anime search ranks by popularity and caps a page at 20 hits,
//! so a broad query buries new or minor shows several pages deep.
//! A page carries where the next one starts, so the caller can walk
//! on without knowing the page size.

use serde::{Deserialize, Serialize};

use super::kitsu::KitsuAnimeRef;

/// The order a search's pages come in. Kitsu ranks a text search by
/// relevance unless asked otherwise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchOrder {
    /// Kitsu's own ranking.
    #[default]
    Relevance,
    /// Newest first by start date.
    Year,
    /// Highest average rating first.
    Rating,
}

impl SearchOrder {
    /// The name this order goes by in a cache key.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Year => "year",
            Self::Rating => "rating",
        }
    }
}

/// A search page: its hits, and the raw offset of the following page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitsuSearchPage {
    /// The page's hits, music videos already dropped.
    pub hits: Vec<KitsuAnimeRef>,
    /// Where the next page starts, `None` on the last one: the app's row
    /// cursor, which the client turns into Kitsu's `page[number]`.
    /// Counted in Kitsu's own rows, before any hit was dropped here.
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
