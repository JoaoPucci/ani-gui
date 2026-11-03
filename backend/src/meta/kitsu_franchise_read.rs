//! The relations read the franchise walk works from: every entry Kitsu
//! relates an anime to, with what a watch-order strip shows of each
//! related anime (title, format, start, count, status, artwork), paged
//! past Kitsu's twenty per page.
//!
//! Separate from [`super::relations`] (`media_relationships`), which
//! asks one page with no titles or artwork and feeds the tracker map;
//! the two are cached under families of their own.

use serde::{Deserialize, Serialize};

use super::KitsuClient;
use crate::error::{AniError, Result};

/// What the franchise surfaces show of one anime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FranchiseEntry {
    /// Its Kitsu id.
    pub id: String,
    /// Kitsu's canonical title.
    pub title: String,
    /// Its subtype as Kitsu serves it: `TV`, `movie`, `OVA`, ….
    pub subtype: Option<String>,
    /// The day it started, `YYYY-MM-DD`.
    pub start: Option<String>,
    /// Its episode count.
    pub episodes: Option<u32>,
    /// Its status: `finished`, `current`, ….
    pub status: Option<String>,
    /// Its poster: the medium size, else small, else the original.
    pub poster: Option<String>,
    /// Its cover: the large size, else the original.
    pub cover: Option<String>,
}

/// One relation of an entry's listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FranchiseRelation {
    /// How Kitsu relates it: `sequel`, `side_story`, `summary`, ….
    pub role: String,
    /// The related resource's type: `anime` or `manga`.
    pub kind: String,
    /// Its Kitsu id.
    pub id: String,
    /// Its subtype.
    pub subtype: Option<String>,
    /// What the surfaces show of it, for an anime.
    pub entry: Option<FranchiseEntry>,
}

/// Every relation Kitsu lists for an entry, as far as the pages read
/// reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FranchiseListing {
    /// The relations, in Kitsu's order across the pages read.
    pub relations: Vec<FranchiseRelation>,
    /// Whether they are every relation Kitsu counts: false when the
    /// page cap cut the listing short.
    pub complete: bool,
}

/// One page of a listing: its relations, how many rows it held (a
/// relation pointing nowhere included) and how many Kitsu counts in
/// all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FranchisePage {
    /// The relations that point somewhere.
    pub relations: Vec<FranchiseRelation>,
    /// The rows the page held.
    pub read: usize,
    /// Kitsu's `meta.count`.
    pub count: Option<usize>,
}

/// Relations asked per page — the most Kitsu serves at once.
pub const FRANCHISE_PAGE_LIMIT: usize = 20;
/// Pages read at most per listing. A listing longer than this is read
/// as incomplete.
pub const FRANCHISE_MAX_PAGES: usize = 5;
/// The fields asked of each related anime.
pub const FRANCHISE_ANIME_FIELDS: &str =
    "canonicalTitle,subtype,status,episodeCount,startDate,posterImage,coverImage";

/// Parse one `media-relationships?include=destination` page of the
/// franchise read. A relation pointing nowhere is dropped; one whose
/// destination is not included, or an anime that leaves out a field it
/// was asked for, is no answer.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body is not that document.
pub fn parse_franchise_page(body: &[u8]) -> Result<FranchisePage> {
    let _ = body;
    Err(AniError::ParseFailed {
        detail: "kitsu franchise relations parse: not yet read".into(),
    })
}

/// Whether another page is worth asking for, after `pages` pages that
/// held `rows` rows in all, the last of them `last_read`, with Kitsu
/// counting `count`.
#[must_use]
pub fn more_pages(pages: usize, rows: usize, last_read: usize, count: Option<usize>) -> bool {
    let _ = (pages, rows, last_read, count);
    false
}

/// Whether `rows` rows read are every relation Kitsu counts.
#[must_use]
pub fn listing_complete(rows: usize, count: Option<usize>) -> bool {
    let _ = (rows, count);
    false
}

impl KitsuClient {
    /// Every relation Kitsu lists for `anime_id`, with each related
    /// anime's title, format, start, count, status and artwork, paged
    /// while Kitsu counts more, up to [`FRANCHISE_MAX_PAGES`].
    ///
    /// # Errors
    /// [`AniError::Network`] when Kitsu cannot be reached,
    /// [`AniError::Upstream`] for a non-success status, and the
    /// parser's errors.
    pub async fn franchise_relations(&self, anime_id: &str) -> Result<FranchiseListing> {
        let _ = (anime_id, &self.http, &self.base);
        Ok(FranchiseListing {
            relations: Vec::new(),
            complete: false,
        })
    }
}

#[cfg(test)]
#[path = "kitsu_franchise_read_test.rs"]
mod tests;

#[cfg(test)]
#[path = "kitsu_franchise_read_prop_test.rs"]
mod prop_tests;
