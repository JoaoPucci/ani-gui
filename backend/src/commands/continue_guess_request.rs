//! What the renderer sends the guess's rules, and their answer.

use crate::meta::kitsu::KitsuAnimeRef;

/// A hit as the renderer sends it: the fields the rules read, any of
/// them but the id absent on an answer cached before Kitsu's titles
/// were kept, and then read as carrying none.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SentHit {
    /// The Kitsu id.
    pub id: String,
    /// The canonical title.
    #[serde(default)]
    pub canonical_title: String,
    /// The localized titles.
    #[serde(default)]
    pub titles: std::collections::HashMap<String, String>,
    /// The abbreviations.
    #[serde(default)]
    pub abbreviated_titles: Vec<String>,
    /// The slug.
    #[serde(default)]
    pub slug: Option<String>,
    /// The episode count.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// The subtype.
    #[serde(default)]
    pub subtype: Option<String>,
}

impl From<SentHit> for KitsuAnimeRef {
    fn from(h: SentHit) -> Self {
        Self {
            id: h.id,
            canonical_title: h.canonical_title,
            titles: h.titles,
            abbreviated_titles: h.abbreviated_titles,
            slug: h.slug,
            synopsis: None,
            start_date: None,
            end_date: None,
            episode_count: h.episode_count,
            average_rating: None,
            subtype: h.subtype,
            status: None,
            age_rating: None,
            popularity_rank: None,
            poster_image: None,
            cover_image: None,
            anilist_banner: None,
        }
    }
}

/// What the renderer asks: the row's search title, the episode it
/// reached, and the hits its search or slug lookup returned.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuledOutRequest {
    /// The row's search title.
    pub title: String,
    /// The episode the row reached, in the provider's numbering.
    pub episode: f64,
    /// The hits.
    pub hits: Vec<SentHit>,
}

/// The ids of the request's hits its row rules out.
#[must_use]
pub fn answer(req: RuledOutRequest) -> Vec<String> {
    let hits: Vec<KitsuAnimeRef> = req.hits.into_iter().map(Into::into).collect();
    super::ruled_out_ids(&req.title, req.episode, &hits)
}
