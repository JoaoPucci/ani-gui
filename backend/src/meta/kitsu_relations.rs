//! Kitsu's two reads the episode map needs beyond an entry's own
//! mappings: every Kitsu entry mapped to one tracker id, and an entry's
//! relationships with the related anime inlined. Split from [`super`]
//! so each file stays inside the CRAP gate's per-file bar.

use super::*;

/// One relationship Kitsu records from an anime to another anime.
#[derive(Debug, Clone, PartialEq)]
pub struct KitsuRelation {
    /// Kitsu's role: `sequel`, `prequel`, `side_story`, `other`, …
    pub role: String,
    /// The related anime, as Kitsu inlined it.
    pub anime: KitsuAnimeRef,
}

impl KitsuClient {
    /// Every Kitsu anime mapped to `external_id` on `site`
    /// (`myanimelist/anime`, `anilist/anime`), each inlined in full.
    /// Several Kitsu entries can map to one tracker entry — the Attack
    /// on Titan Final Chapters specials both map to one MyAnimeList id
    /// — and the episode map reads them all; [`Self::lookup_by_mal_id`]
    /// takes the first for a card.
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn anime_by_external_id(
        &self,
        _site: &str,
        _external_id: u32,
    ) -> Result<Vec<KitsuAnimeRef>> {
        Err(AniError::Metadata)
    }

    /// The relationships Kitsu records from anime `kitsu_id` to other
    /// anime, with each destination inlined so a sequel's count and run
    /// cost no further request. Relationships to manga, and any whose
    /// destination Kitsu did not inline, are dropped.
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn media_relationships(&self, _kitsu_id: &str) -> Result<Vec<KitsuRelation>> {
        Err(AniError::Metadata)
    }
}

#[cfg(test)]
#[path = "kitsu_relations_test.rs"]
mod tests;
