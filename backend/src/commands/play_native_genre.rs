//! The pick's genre gate: no listing its own entry page files under
//! Hentai is ever a pick, and none whose page lists no genre is taken
//! as safe — split from `play_native_titled` so each file stays inside
//! the complexity ratchet's per-file bar.
//!
//! A dated entry's year filter reads every considered hit's page and
//! drops both before scoring (`play_native_year`). An undated entry
//! reads no page there, so the gate reads the pages of the listings a
//! pick would serve — the hit, and every version beside it — once the
//! pick is made: one request for most picks. A listing that fails is
//! left out of the pick's bounded head, no other hit taking its place,
//! as the year filter leaves a hit out, and the head is picked again;
//! [`Memo`] answers that pick's probes and pages from the first, so it
//! asks the provider for nothing the first already asked.

use crate::error::{AniError, Result};
use crate::scraper::provider::{BrowseHit, Provider};

use super::play_native::{pick_over, PickedShow};
use super::play_native_title_marker::EntryTitles;
use super::play_native_year::{genre_of, Genre};

#[path = "play_native_genre_memo.rs"]
mod memo;
pub(crate) use memo::Memo;

/// [`super::play_native::pick_candidate`] told every title the
/// requested entry goes by — the canonical title and its fallbacks, not
/// only the one searched — which the split-entry detection reads to
/// tell the entry from its parts.
///
/// No listing its own entry page files under Hentai is picked, nor one
/// whose page lists no genre: a dated entry's year filter leaves both
/// out of the head, and an undated entry's pick reads the page of each
/// listing it would serve and, when one fails, is made again without it
/// (see the module docs). A pool a listing that could have been picked
/// was left out of for a page that listed no genre is never the clean
/// miss.
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
    loop {
        let picked = pick_over(
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
        .map_err(|e| gate.miss(e))?;
        // A dated entry's year filter read the page of every listing
        // the pick could serve, and left out each the genre refuses.
        // An undated pick serves its hit and its versions alone: a
        // stitched split, whose later parts it would serve too, needs
        // every part's year to match, which an undated entry has none
        // of.
        debug_assert!(year.is_some() || !picked.stitched);
        if year.is_some() || !refused_served(&client, &picked, &mut gate).await? {
            return Ok(picked);
        }
        // Each round leaves out at least one listing of the bounded
        // head, and no other hit takes its place, so the walk ends.
    }
}

/// What the genre gate has learned of one pool: the listings their own
/// pages refuse, which the pick leaves out of its head as the year
/// filter leaves a hit out, and whether a listing that could have been
/// picked was left out because its page listed no genre.
#[derive(Debug, Default)]
pub(crate) struct GenreRefusals {
    /// The slugs of the listings the gate refused.
    pub(crate) excluded: Vec<String>,
    /// A listing that could have been picked was left out for a page
    /// that listed no genre: the pool's miss is then no clean one
    /// ([`unread_miss`]).
    pub(crate) unread: bool,
    /// A hit the head held only as evidence was left out for a page
    /// that listed no genre: the refusal by name it may have made is
    /// no clean miss either ([`unread_miss`]).
    pub(crate) unread_evidence: bool,
}

impl GenreRefusals {
    /// `head` without the listings the gate refused, as the year
    /// filter leaves a hit out: no other hit takes their place.
    pub(crate) fn leave_out(&self, head: &mut Vec<BrowseHit>) {
        head.retain(|h| !self.excluded.contains(&h.slug));
    }

    /// Notes which hits of `head` the year filter left out for a page
    /// that lists no genre (`unread`, by slug): one that could have been
    /// picked — admitted by the entry's titles and not another entry,
    /// each read past its version tag, as the pick reads them — makes
    /// the miss the dead end; one the head holds only as evidence never
    /// could have been, but its refusal by name, had the page been read,
    /// may have been the verdict, so it keeps the miss from being clean.
    pub(crate) fn note_unread(
        &mut self,
        head: &[BrowseHit],
        unread: &[String],
        entry: EntryTitles<'_>,
    ) {
        for h in head.iter().filter(|h| unread.contains(&h.slug)) {
            if entry.admits_as_read(&h.title) && !entry.another_entry_as_read(&h.title) {
                self.unread = true;
            } else {
                self.unread_evidence = true;
            }
        }
    }

    /// The pool's verdict `verdict`, as what the gate left out leaves
    /// it ([`unread_miss`]).
    pub(crate) fn miss(&self, verdict: AniError) -> AniError {
        unread_miss(verdict, self.unread, self.unread_evidence)
    }
}

/// Whether a listing `picked` would serve — its hit, then each
/// version's — is one its own page refuses: filed under Hentai, or
/// listing no genre, which sets `gate.unread`. Each one refused joins
/// `gate.excluded`. A page that answers says nothing new is read once
/// ([`Memo`]); one whose genre is unread is logged, since a markup
/// change that loses the row would refuse every listing.
///
/// # Errors
/// A page's provider block or transport failure, verbatim: as in the
/// year filter, probing the next page would turn one block into a
/// burst.
pub(crate) async fn refused_served<P: Provider + ?Sized>(
    client: &P,
    picked: &PickedShow,
    gate: &mut GenreRefusals,
) -> Result<bool> {
    let mut refused = false;
    let served = std::iter::once(&picked.hit).chain(picked.versions.iter().map(|v| &v.hit));
    for hit in served {
        if gate.excluded.contains(&hit.slug) {
            continue;
        }
        match genre_of(&client.detail_page(&hit.slug).await?) {
            Genre::Listed => continue,
            Genre::Hentai => {}
            Genre::Unread => {
                tracing::warn!(slug = %hit.slug, "pick: the entry page lists no genre; not taken as safe");
                gate.unread = true;
            }
        }
        gate.excluded.push(hit.slug.clone());
        refused = true;
    }
    Ok(refused)
}

/// The pool's verdict once listings whose pages listed no genre were
/// left out of it. With one that could have been picked among them
/// (`unread`), a miss — the clean one, or the refusal by name — is the
/// answered dead end on a listing that may have been the entry's,
/// `Upstream { status: 404 }`, as a dead listing's is: never the
/// absence the availability cache persists, nor the refusal that says
/// the catalogue lists nothing named for the entry. With only evidence
/// among them (`unread_evidence`), the clean miss is the refusal by
/// name the evidence may have made: no listing that could have been
/// picked went unread, but the pool is not known to hold nothing the
/// count accepts. Any other verdict, and every verdict of a pool whose
/// pages all said what they file their listings under, stands as it is.
#[must_use]
pub(crate) fn unread_miss(verdict: AniError, unread: bool, unread_evidence: bool) -> AniError {
    match verdict {
        AniError::NoResults | AniError::RefusedByName if unread => {
            AniError::Upstream { status: 404 }
        }
        AniError::NoResults if unread_evidence => AniError::RefusedByName,
        other => other,
    }
}

#[cfg(test)]
#[path = "play_native_genre_test.rs"]
mod tests;
