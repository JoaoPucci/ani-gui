//! The pick's genre gate — split from `play_native_titled` so each file
//! stays inside the complexity ratchet's per-file bar. The pick runs
//! over a [`Memo`] of the provider, and what the gate has refused over
//! a pool ([`GenreRefusals`]) is left out of the pick's head.

use crate::error::{AniError, Result};
use crate::scraper::provider::{BrowseHit, Provider};

use super::play_native::{pick_over, PickedShow};

#[path = "play_native_genre_memo.rs"]
mod memo;
pub(crate) use memo::Memo;

/// [`super::play_native::pick_candidate`] told every title the
/// requested entry goes by — the canonical title and its fallbacks, not
/// only the one searched — which the split-entry detection reads to
/// tell the entry from its parts.
///
/// # Errors
/// As [`super::play_native::pick_candidate`].
pub async fn pick_candidate_titled<P: Provider + ?Sized>(
    client: &P,
    hits: &[BrowseHit],
    expected: Option<u32>,
    search_title: &str,
    entry_titles: &[&str],
    year: Option<u32>,
    subtype: Option<&str>,
) -> Result<PickedShow> {
    let client = Memo::new(client);
    let mut gate = GenreRefusals::default();
    pick_over(
        &client,
        hits,
        expected,
        search_title,
        entry_titles,
        year,
        subtype,
        &mut gate,
    )
    .await
    .map_err(|e| unread_miss(e, gate.unread))
}

/// What the genre gate has learned of one pool: the listings it
/// refused, which the pick leaves out of its head as the year filter
/// leaves a hit out, and whether a listing that could have been picked
/// was left out because its page listed no genre.
#[derive(Debug, Default)]
pub(crate) struct GenreRefusals {
    /// The slugs of the listings the gate refused.
    pub(crate) excluded: Vec<String>,
    /// A listing that could have been picked was left out for a page
    /// that listed no genre: the pool's miss is then no clean one
    /// ([`unread_miss`]).
    pub(crate) unread: bool,
}

impl GenreRefusals {
    /// `head` without the listings the gate refused, as the year
    /// filter leaves a hit out: no other hit takes their place.
    pub(crate) fn leave_out(&self, head: &mut Vec<BrowseHit>) {
        head.retain(|h| !self.excluded.contains(&h.slug));
    }
}

/// The pool's verdict once a listing whose page listed no genre was
/// left out of it (`unread`): a miss — the clean one, or the refusal by
/// name — is the answered dead end on a listing that may have been the
/// entry's, `Upstream { status: 404 }`, as a dead listing's is: never
/// the absence the availability cache persists, nor the refusal that
/// says the catalogue lists nothing named for the entry. Any other
/// verdict, and every verdict of a pool whose pages all said what they
/// file their listings under, stands as it is.
#[must_use]
pub(crate) fn unread_miss(verdict: AniError, unread: bool) -> AniError {
    match verdict {
        AniError::NoResults | AniError::RefusedByName if unread => {
            AniError::Upstream { status: 404 }
        }
        other => other,
    }
}

#[cfg(test)]
#[path = "play_native_genre_test.rs"]
mod tests;
