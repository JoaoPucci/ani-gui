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
    /// The row's show key (`hianime:jujutsu-kaisen-237`). A request
    /// without one is read as a bare row's: its episode rules nothing
    /// out, and its key carries no words. The renderer always sends
    /// it.
    #[serde(default)]
    pub key: String,
    /// The row's search title.
    pub title: String,
    /// The episode the row reached, as the history route reads it: the
    /// listing's own number, or the Kitsu entry's where the row's
    /// numbering was stamped.
    pub episode: f64,
    /// The hits.
    pub hits: Vec<SentHit>,
    /// Whether the hits are the answer of the row's title search, which
    /// the rules that say whether a search answered the row at all
    /// ([`super::refutes`]) read as a whole. A lone hit — a stored title
    /// match, a slug lookup's entry — is judged by the other rules
    /// alone, since one weakly refuted hit says nothing about a search.
    #[serde(default)]
    pub search: bool,
}

/// The ids of the request's hits its row rules out. The episode counts
/// only for a row a provider the app walks wrote: hianime numbers each
/// listing from its first episode, where anidb.app numbered a later cour
/// on the franchise's count, and a bare row it wrote still speaks that
/// number unless its numbering was stamped. The key's words count for
/// any key whose slug carries them, as both providers' do, and only
/// for a search's answer.
#[must_use]
pub fn answer(req: RuledOutRequest) -> Vec<String> {
    let key = crate::scraper::provider::ShowKey::parse(&req.key);
    let episode = key.provider.is_some().then_some(req.episode);
    let key_words = key.search_term().unwrap_or_default();
    let hits: Vec<KitsuAnimeRef> = req.hits.into_iter().map(Into::into).collect();
    if req.search {
        return super::ruled_out_ids(&req.title, &key_words, episode, &hits);
    }
    hits.iter()
        .filter(|h| super::ruled_out(&req.title, episode, h))
        .map(|h| h.id.clone())
        .collect()
}
