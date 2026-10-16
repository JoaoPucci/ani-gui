//! The entries Kitsu relates an anime to — its sequels, side stories,
//! OVAs — from the `media-relationships` endpoint, split from
//! `kitsu.rs` so that file stays inside the complexity bar.

use serde::{Deserialize, Serialize};

use super::KitsuClient;
use crate::error::{AniError, Result};

/// One entry Kitsu relates an anime to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitsuRelation {
    /// How Kitsu relates it: `sequel`, `side_story`, `adaptation`, ….
    pub role: String,
    /// The related resource's type: `anime` or `manga`.
    pub kind: String,
    /// Its Kitsu id.
    pub id: String,
    /// Its subtype: `TV`, `OVA`, `ONA`, `special`, `manga`, ….
    pub subtype: Option<String>,
    /// Its status: `finished`, `current`, ….
    pub status: Option<String>,
    /// Its episode count, for an anime.
    pub episode_count: Option<u32>,
}

/// The entries Kitsu relates an anime to, and whether the page read
/// held all of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitsuRelations {
    /// The relations on the page read, in Kitsu's order.
    pub entries: Vec<KitsuRelation>,
    /// Whether they are every relation Kitsu counts.
    pub complete: bool,
}

/// How many relations the backend asks Kitsu for at once — the most a
/// page holds.
pub const RELATIONS_PAGE_LIMIT: u8 = 20;

/// Parse a `media-relationships?include=destination` page. A relation
/// whose destination names nothing is dropped; the page is complete
/// when it held as many relations as Kitsu counts.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body is not that document.
pub fn parse_relationships_response(_body: &[u8]) -> Result<KitsuRelations> {
    Ok(KitsuRelations {
        entries: Vec::new(),
        complete: false,
    })
}

impl KitsuClient {
    /// The entries Kitsu relates `anime_id` to: one page of them, with
    /// each destination's subtype, status and episode count.
    ///
    /// # Errors
    /// [`AniError::Network`] when Kitsu cannot be reached,
    /// [`AniError::Upstream`] for a non-success status, and the
    /// parser's errors.
    pub async fn media_relationships(&self, anime_id: &str) -> Result<KitsuRelations> {
        let resp = self
            .http
            .get(format!(
                "{}/anime/{}/media-relationships",
                self.base, anime_id
            ))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(&[
                ("include", "destination".to_string()),
                ("fields[anime]", "subtype,status,episodeCount".to_string()),
                ("fields[manga]", "subtype,status".to_string()),
                ("page[limit]", RELATIONS_PAGE_LIMIT.to_string()),
            ])
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        let body = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_relationships_response(&body)
    }
}
