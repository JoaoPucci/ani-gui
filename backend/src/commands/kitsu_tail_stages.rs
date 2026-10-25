//! The order the evidence reader reads in ([`super`]): each read only
//! while what is in hand leaves the reading open, so a refusal it
//! settles stands whatever a later read would do. Split from the reads
//! themselves so each file stays inside the complexity bar.

use super::read::{
    anilist_count, entry_tail, is_tv, page_of, related, sequel_episodes, tail_episodes, unpaged,
};
use super::KitsuTail;
use crate::commands::play_native_appended::{
    aligned, anilist_ends_at, explained, is_announced, kitsu_ends_at, leaves_names,
    misaligned_so_far, named_alone, past_named_as_a_last_episode, rows_end_at, unlike_the_show_s,
    EntryTail, Related, ALIGNED,
};
use crate::meta::anilist_airing::AiringStatus;
use crate::meta::kitsu::KitsuRelation;
use crate::scraper::provider::EpisodeRef;

/// Whether the Kitsu rows read so far already refuse the reading, so
/// no further page can undo it: see [`KitsuTail::kitsu_rows`].
fn settled(listing: &[EpisodeRef], count: u32, past: &[EpisodeRef], tail: &EntryTail) -> bool {
    !rows_end_at(tail, count)
        || !unlike_the_show_s(past, tail)
        || misaligned_so_far(listing, count, tail)
}

impl KitsuTail<'_> {
    /// What says whether the show ends at `count`, read in order while
    /// what is in hand leaves the reading open: the listing's rows past
    /// the count (`past`) have names of their own, its names for the
    /// count's last episodes could line up with Kitsu's
    /// ([`misaligned_so_far`] on the listing alone), and no row past
    /// the count carries one of them ([`past_named_as_a_last_episode`]);
    /// Kitsu's entry ends the show at the count; AniList's answer does;
    /// Kitsu's pages list nothing past it, name its last episodes as
    /// `listing` does, and name no row past it. Returns the tail so far
    /// and whether the reading is still open; `None` when a read failed
    /// first.
    pub(super) async fn end_of_show(
        &self,
        listing: &[EpisodeRef],
        count: u32,
        past: &[EpisodeRef],
    ) -> Option<(EntryTail, bool)> {
        // The listing alone can settle it, before anything is read.
        if !named_alone(past)
            || misaligned_so_far(listing, count, &EntryTail::default())
            || past_named_as_a_last_episode(listing, count, past)
        {
            return Some((EntryTail::default(), false));
        }
        let detail = crate::commands::kitsu::kitsu_anime_detail(self.state, self.kitsu_id)
            .await
            .ok()?;
        let mut tail = entry_tail(
            detail.status.as_deref(),
            detail.episode_count,
            &AiringStatus::default(),
            Vec::new(),
        );
        if !kitsu_ends_at(&tail, count) {
            return Some((tail, false));
        }
        let airing = crate::commands::airing::airing_get(self.state, self.kitsu_id)
            .await
            .ok()?;
        tail.anilist_count = anilist_count(&airing);
        if !anilist_ends_at(&tail, count) {
            return Some((tail, false));
        }
        self.kitsu_rows(listing, count, past, tail).await
    }

    /// `tail` with Kitsu's rows: the pages holding the count's last
    /// episodes, then the page after them, each read only while the
    /// rows before it leave the reading open. Kitsu listing an episode
    /// past the count, a listing row past it named as one of the show's
    /// own episodes, or the count's last episodes read so far already
    /// unable to line up with the listing ([`misaligned_so_far`]) is a
    /// refusal no further page undoes.
    /// The page after is the one that says nothing follows the count:
    /// Kitsu's pages are not always dense, and the placeholder rows the
    /// reader drops hide how full a page was.
    async fn kitsu_rows(
        &self,
        listing: &[EpisodeRef],
        count: u32,
        past: &[EpisodeRef],
        mut tail: EntryTail,
    ) -> Option<(EntryTail, bool)> {
        let first = page_of(count.saturating_sub(ALIGNED - 1).max(1));
        for page in first..=page_of(count) + 1 {
            tail.episodes.extend(tail_episodes(self.page(page).await?));
            if settled(listing, count, past, &tail) {
                return Some((tail, false));
            }
        }
        // Nothing settled, so only the alignment is left to say.
        let open = aligned(listing, count, &tail);
        Some((tail, open))
    }

    /// `tail` with the entries Kitsu relates the show to: read whole,
    /// then, while they explain the `past` rows, each aired TV entry's
    /// first page, until a page refuses the rows their names.
    pub(super) async fn relations_of(
        &self,
        mut tail: EntryTail,
        past: &[EpisodeRef],
    ) -> Option<EntryTail> {
        let relations = self.read_relations().await?;
        tail.related_complete = relations.complete;
        tail.related = unpaged(&relations.entries);
        if !explained(past.len(), &tail) {
            return Some(tail);
        }
        tail.related = self.with_first_pages(relations.entries, past).await?;
        Some(tail)
    }

    /// `entries` as the evidence reads them, each TV entry that has
    /// aired, whatever the role, with its first page of episodes, until
    /// one leaves a row past the count without its name; the pages
    /// after that one are not asked for. An entry only announced has
    /// aired nothing, and the rule passes it over: its page is not
    /// asked for either, since an answer the read does not need could
    /// still fail it.
    async fn with_first_pages(
        &self,
        entries: Vec<KitsuRelation>,
        past: &[EpisodeRef],
    ) -> Option<Vec<Related>> {
        let mut out = Vec::new();
        let mut open = true;
        for r in entries {
            let aired = !is_announced(r.status.as_deref());
            let first_episodes = if open && is_tv(&r) && aired {
                let page = crate::commands::kitsu::kitsu_episode_page(self.state, &r.id, 1)
                    .await
                    .ok()?;
                let page = sequel_episodes(page);
                open = leaves_names(past, &page);
                Some(page)
            } else {
                None
            };
            out.push(related(r, first_episodes));
        }
        Some(out)
    }
}
