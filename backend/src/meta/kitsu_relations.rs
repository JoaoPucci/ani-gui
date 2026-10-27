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

#[derive(Deserialize)]
struct Doc {
    data: Vec<Edge>,
    #[serde(default)]
    included: Vec<Node>,
    #[serde(default)]
    meta: Option<Meta>,
}

#[derive(Deserialize)]
struct Meta {
    count: Option<usize>,
}

#[derive(Deserialize)]
struct Edge {
    attributes: EdgeAttributes,
    relationships: EdgeLinks,
}

#[derive(Deserialize)]
struct EdgeAttributes {
    role: String,
}

#[derive(Deserialize)]
struct EdgeLinks {
    destination: Destination,
}

#[derive(Deserialize)]
struct Destination {
    // Null when the relation points nowhere; missing, it says nothing.
    data: crate::meta::present::Present<Ref>,
}

#[derive(Deserialize, PartialEq, Eq)]
struct Ref {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    attributes: NodeAttributes,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeAttributes {
    subtype: Option<String>,
    status: Option<String>,
    episode_count: Option<u32>,
}

/// Parse a `media-relationships?include=destination` page. A relation
/// whose destination names nothing is dropped; the page is complete
/// when it held as many relations as Kitsu counts.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body is not that document.
pub fn parse_relationships_response(body: &[u8]) -> Result<KitsuRelations> {
    let doc: Doc = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("kitsu relationships parse: {e}"),
    })?;
    let read = doc.data.len();
    let mut entries = Vec::new();
    for edge in doc.data {
        let Some(dst) = edge.relationships.destination.data.into_inner() else {
            continue;
        };
        // The page is asked for with its destinations included: one
        // that is not says nothing of what the related entry is, and
        // reading it as an entry of no format or status would decide
        // what the relation explains on nothing.
        let node = doc
            .included
            .iter()
            .find(|n| n.kind == dst.kind && n.id == dst.id)
            .ok_or_else(|| AniError::ParseFailed {
                detail: format!(
                    "kitsu relationships parse: {} {} not included",
                    dst.kind, dst.id
                ),
            })?;
        entries.push(KitsuRelation {
            role: edge.attributes.role,
            subtype: node.attributes.subtype.clone(),
            status: node.attributes.status.clone(),
            episode_count: node.attributes.episode_count,
            kind: dst.kind,
            id: dst.id,
        });
    }
    let complete = doc.meta.and_then(|m| m.count) == Some(read);
    Ok(KitsuRelations { entries, complete })
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

#[cfg(test)]
#[path = "kitsu_relations_test.rs"]
mod tests;

#[cfg(test)]
#[path = "kitsu_relations_prop_test.rs"]
mod prop_tests;
