//! Asking AniList's `Page` for many ids at once, in pages it answers
//! whole: the walk the batched translation by MAL id
//! ([`super::anilist::media_listed_for_mals`]) and the batched airing
//! queries ([`super::anilist_airing`]) share.
//!
//! `Page` holds [`PAGE_CAP`] media. A chunk of that many ids can still
//! fill more than a page where several media answer one id, and the
//! page then says it was cut (`hasNextPage`). Such a chunk is split in
//! two and each half asked again, in order, until every page asked is
//! whole; an id whose page is cut even alone is handed to the caller,
//! which decides what that means.

use std::future::Future;

use crate::error::Result;

/// The most media AniList's `Page` holds, and so the most ids a chunk
/// asks for.
pub const PAGE_CAP: usize = 50;

/// What a page asked for a chunk of ids came back as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page<T> {
    /// The page held every media the chunk asked for.
    Whole(T),
    /// AniList cut the page: more media answer the chunk than it holds.
    Cut,
}

/// Every whole page `ids` is asked in, in order: chunks of [`PAGE_CAP`],
/// each asked with `ask`; a cut chunk split in two and each half asked
/// in its place, the first half first; an id cut even alone handed to
/// `cut_alone`, whose error ends the walk. Empty `ids` ask nothing. An
/// error from `ask` ends the walk with it.
///
/// # Errors
/// The first error `ask` or `cut_alone` returns.
pub async fn whole_pages<'a, T, F, Fut, C>(
    ids: &'a [u32],
    mut ask: F,
    mut cut_alone: C,
) -> Result<Vec<T>>
where
    F: FnMut(&'a [u32]) -> Fut,
    Fut: Future<Output = Result<Page<T>>>,
    C: FnMut(u32) -> Result<()>,
{
    let mut pages = Vec::new();
    let mut chunks: Vec<&'a [u32]> = ids.chunks(PAGE_CAP).rev().collect();
    while let Some(chunk) = chunks.pop() {
        match ask(chunk).await? {
            Page::Whole(page) => pages.push(page),
            Page::Cut => match chunk {
                [one] => cut_alone(*one)?,
                _ => {
                    let (first, rest) = chunk.split_at(chunk.len() / 2);
                    chunks.push(rest);
                    chunks.push(first);
                }
            },
        }
    }
    Ok(pages)
}
