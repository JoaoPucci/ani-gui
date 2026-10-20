//! The evidence reader's reads and the mapping of what they return
//! ([`super`]), split from it so each file stays inside the complexity
//! bar.

use super::{KitsuTail, KITSU_PAGE};
use crate::commands::play_native_appended::{
    EntryTail, Related, SequelEpisode, TailEpisode, ALIGNED,
};
use crate::meta::anilist_airing::AiringStatus;
use crate::meta::kitsu::{KitsuEpisode, KitsuRelation};
use crate::scraper::provider::EpisodeRef;

/// The 1-based Kitsu page episode `n` is on.
fn page_of(n: u32) -> u32 {
    n.saturating_sub(1) / KITSU_PAGE + 1
}

/// The Kitsu pages that hold the count's last [`ALIGNED`] episodes,
/// and the one an episode past the count would be on: the page after
/// when the count ends a page.
#[must_use]
pub fn tail_pages(count: u32) -> Vec<u32> {
    let first = count.saturating_sub(ALIGNED - 1).max(1);
    let mut pages: Vec<u32> = (page_of(first)..=page_of(count)).collect();
    if count % KITSU_PAGE == 0 {
        pages.push(page_of(count) + 1);
    }
    pages
}

/// The tail Kitsu's `status` and `count`, AniList's `airing` answer
/// and the rows of the pages [`tail_pages`] names give. AniList's
/// count is the one it gives a show it calls finished: an aired count
/// with nothing scheduled after it.
#[must_use]
pub fn entry_tail(
    status: Option<&str>,
    count: Option<u32>,
    airing: &AiringStatus,
    episodes: Vec<KitsuEpisode>,
) -> EntryTail {
    EntryTail {
        finished: status == Some("finished"),
        count,
        anilist_count: anilist_count(airing),
        episodes: tail_episodes(episodes),
        read_past_count: true,
        related: Vec::new(),
        related_complete: false,
    }
}

/// The count AniList gives a show it calls finished: an aired count
/// with nothing scheduled after it.
pub(super) fn anilist_count(airing: &AiringStatus) -> Option<u32> {
    airing
        .aired
        .filter(|n| *n > 0 && airing.next_episode.is_none())
}

/// Kitsu's rows as the evidence reads them: each with a number.
pub(super) fn tail_episodes(episodes: Vec<KitsuEpisode>) -> Vec<TailEpisode> {
    episodes
        .into_iter()
        .filter_map(|e| {
            Some(TailEpisode {
                number: e.number?,
                title: e.canonical_title,
                airdate: e.airdate,
            })
        })
        .collect()
}

/// The relations as the evidence reads them before any page is asked
/// for: enough to say whether they explain the rows past the count.
pub(super) fn unpaged(entries: &[KitsuRelation]) -> Vec<Related> {
    entries.iter().map(|r| related(r.clone(), None)).collect()
}

/// Whether `r` is a TV entry, in whatever role Kitsu relates it: an
/// entry whose first episodes the rows past the count are checked
/// against.
fn is_tv(r: &KitsuRelation) -> bool {
    r.kind == "anime" && r.subtype.as_deref() == Some("TV")
}

/// A related entry's page of episodes as the evidence reads it: each
/// row with a number, by that number, with the title Kitsu gives it.
fn sequel_episodes(page: Vec<KitsuEpisode>) -> Vec<SequelEpisode> {
    page.into_iter()
        .filter_map(|e| {
            Some(SequelEpisode {
                number: e.number?,
                title: e.canonical_title,
            })
        })
        .collect()
}

/// A relation as the evidence reads it.
fn related(r: KitsuRelation, first_episodes: Option<Vec<SequelEpisode>>) -> Related {
    Related {
        anime: r.kind == "anime",
        role: r.role,
        subtype: r.subtype,
        status: r.status,
        episode_count: r.episode_count,
        first_episodes,
    }
}

impl KitsuTail<'_> {
    /// Kitsu's rows on the pages [`tail_pages`] names, and on the page
    /// after the last of them when nothing past the count is on them:
    /// Kitsu's pages are not always dense, and the placeholder rows the
    /// reader drops hide how full a page was, so only the page after
    /// says what follows.
    pub(super) async fn read_pages(&self, count: u32) -> Option<Vec<KitsuEpisode>> {
        let mut episodes = Vec::new();
        let mut last = 0;
        for page in tail_pages(count) {
            episodes.extend(self.page(page).await?);
            last = page;
        }
        // A count that ends a page already read the page after it.
        let looked_ahead = count % KITSU_PAGE == 0;
        if !looked_ahead && episodes.iter().all(|e| e.number.is_none_or(|n| n <= count)) {
            episodes.extend(self.page(last + 1).await?);
        }
        Some(episodes)
    }

    async fn page(&self, page: u32) -> Option<Vec<KitsuEpisode>> {
        crate::commands::kitsu::kitsu_episode_page(self.state, self.kitsu_id, page)
            .await
            .ok()
    }

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
        use crate::commands::play_native_appended::{
            aligned, anilist_ends_at, kitsu_ends_at, named_alone, rows_end_at, unlike_the_show_s,
        };
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
        tail.episodes = tail_episodes(self.read_pages(count).await?);
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
        use crate::commands::play_native_appended::explained;
        let relations = self.read_relations().await?;
        tail.related_complete = relations.complete;
        tail.related = unpaged(&relations.entries);
        if !explained(past.len(), &tail) {
            return Some(tail);
        }
        tail.related = self.with_first_pages(relations.entries, past).await?;
        Some(tail)
    }

    /// The entries Kitsu relates the show to, and whether they are
    /// all of them.
    pub(super) async fn read_relations(&self) -> Option<crate::meta::kitsu::KitsuRelations> {
        crate::commands::kitsu::kitsu_relationships(self.state, self.kitsu_id)
            .await
            .ok()
    }

    /// `entries` as the evidence reads them, each TV entry that has
    /// aired, whatever the role, with its first page of episodes, until
    /// one leaves a row past the count without its name; the pages
    /// after that one are not asked for. An entry only announced has
    /// aired nothing, and the rule passes it over: its page is not
    /// asked for either, since an answer the read does not need could
    /// still fail it.
    pub(super) async fn with_first_pages(
        &self,
        entries: Vec<KitsuRelation>,
        past: &[EpisodeRef],
    ) -> Option<Vec<Related>> {
        use crate::commands::play_native_appended::{is_announced, leaves_names};
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
