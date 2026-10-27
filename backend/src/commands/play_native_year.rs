//! The picker's year-identity filter — split from `play_native` so
//! each file stays inside the complexity ratchet's per-file bar.

use std::collections::HashMap;

use crate::error::Result;
use crate::scraper::provider::{BrowseHit, EntryPage, Provider};

/// The other names each considered hit's entry page gave the show,
/// by slug — read from the pages the year filter already fetched, so
/// only a dated entry's hits carry any.
pub(crate) type PageNames = HashMap<String, Vec<String>>;

/// The genre an entry page files an adult video under.
pub(crate) const HENTAI: &str = "Hentai";

/// What an entry page says of whether its listing may be the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Genre {
    /// The page lists genres, none of them [`HENTAI`].
    Listed,
    /// The page files the listing under [`HENTAI`]: no entry the app
    /// shows is one.
    Hentai,
    /// The page lists no genre — a missing page reads as one, and so
    /// does a page whose markup the parser no longer finds the row
    /// in. Nothing says the listing is not a hentai.
    Unread,
}

/// The [`Genre`] `page` gives its listing.
#[must_use]
pub(crate) fn genre_of(page: &EntryPage) -> Genre {
    if page.genres.iter().any(|g| g.eq_ignore_ascii_case(HENTAI)) {
        Genre::Hentai
    } else if page.genres.is_empty() {
        Genre::Unread
    } else {
        Genre::Listed
    }
}

/// The considered head of `hits`, narrowed by Kitsu's year when it
/// is known: each candidate's detail page names its premiere year,
/// and a known year more than one off Kitsu's excludes the
/// candidate — the identity signal applies even to a lone candidate.
/// Only an exact year counts as positive identity; one year off is
/// tolerated (December premieres straddle catalogue years) without
/// vouching for the candidate.
/// An unknown year (a page without a season link) never excludes on
/// its own, so a markup change that loses the year degrades to
/// year-blind picking. A hit its page files under [`HENTAI`] is
/// excluded the same way a year off is: no entry the app shows is one.
/// So is a hit whose page lists no genre at all ([`Genre::Unread`]) —
/// a 404ing page reads as one — which nothing says is not one; its slug
/// joins `unread`, since a pool whose verdict rests on a page that
/// said nothing is no clean miss (see `play_native_genre`), and a
/// markup change that loses the Genres row empties every dated pool
/// into that dead end rather than taking a listing as safe. A pool whose every
/// candidate carries a known mismatched year, or the genre, contains
/// the show nowhere — that is a rejection
/// ([`crate::error::AniError::NoResults`]), so the walk can try the
/// next alias. Each survivor carries whether its own year positively
/// matched, and every page read leaves the names it gives the show in
/// [`PageNames`] — a survivor's and an excluded hit's alike.
pub(crate) async fn year_filtered<'a, P: Provider + ?Sized>(
    client: &P,
    hits: &'a [BrowseHit],
    year: Option<u32>,
    pages: &mut PageNames,
    unread: &mut Vec<String>,
) -> Result<Vec<(&'a BrowseHit, bool)>> {
    // The caller bounds the head (`probe_head`); every hit given is
    // one the pick means to consider.
    let head = hits.iter();
    let Some(year) = year else {
        return Ok(head.map(|h| (h, false)).collect());
    };
    let mut kept = Vec::new();
    for h in head {
        // `?`: a refusal, rate limit, or transport failure on a
        // detail page is the provider blocking this client — probing
        // the rest of the pool would turn one block into a burst.
        let page = client.detail_page(&h.slug).await?;
        let genre = genre_of(&page);
        if !page.names.is_empty() {
            pages.insert(h.slug.clone(), page.names);
        }
        let confirmed = match page.year {
            // Exact match is positive identity; one year off is the
            // December-premiere allowance — enough to stay in the
            // pool and compete on count, never enough to vouch for
            // a candidate (the rescue and the countless preference
            // key on the flag, and a boundary-tolerated movie must
            // not ride them past a large count mismatch).
            Some(y) if y == year => true,
            Some(y) if y.abs_diff(year) <= 1 => false,
            // Another year's show is not the entry, whatever its
            // page says of its genre.
            Some(_) => continue,
            None => false,
        };
        // A hentai is never the entry: Kitsu serves the app no adult
        // entry, so a hit its own page files under the genre is out,
        // as a hit from another year is — and so is one whose page
        // lists no genre, which nothing says is not one.
        match genre {
            Genre::Listed => kept.push((h, confirmed)),
            Genre::Hentai => {}
            Genre::Unread => {
                tracing::warn!(slug = %h.slug, "pick: the entry page lists no genre; not taken as safe");
                unread.push(h.slug.clone());
            }
        }
    }
    if kept.is_empty() {
        return Err(crate::error::AniError::NoResults);
    }
    Ok(kept)
}
