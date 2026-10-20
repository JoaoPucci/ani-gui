//! The order the evidence reader reads in ([`super`]): each read only
//! while what is in hand leaves the reading open, so a refusal it
//! settles stands whatever a later read would do. Split from the reads
//! themselves so each file stays inside the complexity bar.

use super::read::{
    anilist_count, entry_tail, is_tv, related, sequel_episodes, tail_episodes, unpaged,
};
use super::KitsuTail;
use crate::commands::play_native_appended::{
    aligned, aligned_is_final, anilist_ends_at, explained, is_announced, kitsu_ends_at,
    leaves_names, named_alone, rows_end_at, unlike_the_show_s, EntryTail, Related,
};
use crate::meta::anilist_airing::AiringStatus;
use crate::meta::kitsu::KitsuRelation;
use crate::scraper::provider::EpisodeRef;

impl KitsuTail<'_> {
    /// What says whether the show ends at `count`, read in order while
    /// what is in hand leaves the reading open: the listing's rows past
    /// the count (`past`) have names of their own; Kitsu's entry ends
    /// the show at the count; AniList's answer does; Kitsu's pages list
    /// nothing past it, name its last episodes as `listing` does, and
    /// name no row past it. Returns the tail so far and whether the
    /// reading is still open; `None` when a read failed first.
    pub(super) async fn end_of_show(
        &self,
        listing: &[EpisodeRef],
        count: u32,
        past: &[EpisodeRef],
    ) -> Option<(EntryTail, bool)> {
        if !named_alone(past) {
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

    /// `tail` with Kitsu's rows: the count's pages, then the page after
    /// only while they leave the reading open. A row past the count, a
    /// row past it named as one of the show's own episodes, or the
    /// count's last episodes all read and named otherwise than by the
    /// listing is a refusal no further page undoes.
    async fn kitsu_rows(
        &self,
        listing: &[EpisodeRef],
        count: u32,
        past: &[EpisodeRef],
        mut tail: EntryTail,
    ) -> Option<(EntryTail, bool)> {
        tail.episodes = tail_episodes(self.count_pages(count).await?);
        let settled = !rows_end_at(&tail, count)
            || !unlike_the_show_s(past, &tail)
            || (aligned_is_final(count, &tail) && !aligned(listing, count, &tail));
        if settled {
            return Some((tail, false));
        }
        let after = self.page_after(count, &tail.episodes).await?;
        tail.episodes.extend(tail_episodes(after));
        let open = rows_end_at(&tail, count)
            && aligned(listing, count, &tail)
            && unlike_the_show_s(past, &tail);
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
