//! The franchise read's page parser, split from
//! `kitsu_franchise_read.rs` for the per-file complexity bar.

use serde::Deserialize;

use super::{FranchiseEntry, FranchisePage, FranchiseRelation};
use crate::error::{AniError, Result};
use crate::meta::present::{present, Present};

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
    data: Present<Ref>,
}

#[derive(Deserialize)]
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

#[derive(Deserialize, Default, Clone)]
struct Images {
    #[serde(default)]
    small: Option<String>,
    #[serde(default)]
    medium: Option<String>,
    #[serde(default)]
    large: Option<String>,
    #[serde(default)]
    original: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeAttributes {
    // Asked of every destination: missing is no answer, null is none.
    #[serde(deserialize_with = "present")]
    subtype: Option<String>,
    // Asked of an anime only: `None` when the node does not carry it,
    // `Some(None)` when it carries null.
    #[serde(default, deserialize_with = "carried")]
    canonical_title: Option<Option<String>>,
    #[serde(default, deserialize_with = "carried")]
    status: Option<Option<String>>,
    #[serde(default, deserialize_with = "carried")]
    episode_count: Option<Option<u32>>,
    #[serde(default, deserialize_with = "carried")]
    start_date: Option<Option<String>>,
    // Artwork is shown when there is some: an entry without is drawn
    // without, not refused.
    #[serde(default)]
    poster_image: Option<Images>,
    #[serde(default)]
    cover_image: Option<Images>,
}

/// A field read as carried or not: `Some` with its value (null being
/// `None`) when the node has it; with `serde(default)`, `None` when it
/// does not.
fn carried<'de, D, T>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    present(deserializer).map(Some)
}

fn parse_error(detail: String) -> AniError {
    AniError::ParseFailed {
        detail: format!("kitsu franchise relations parse: {detail}"),
    }
}

/// A field an anime was asked for, or the error naming it.
fn asked<T>(field: Option<Option<T>>, id: &str, name: &str) -> Result<Option<T>> {
    field.ok_or_else(|| parse_error(format!("anime {id} without its {name}")))
}

fn entry(id: &str, attrs: &NodeAttributes) -> Result<FranchiseEntry> {
    let poster = attrs.poster_image.clone().unwrap_or_default();
    let cover = attrs.cover_image.clone().unwrap_or_default();
    Ok(FranchiseEntry {
        id: id.to_string(),
        title: asked(attrs.canonical_title.clone(), id, "title")?.unwrap_or_default(),
        subtype: attrs.subtype.clone(),
        start: asked(attrs.start_date.clone(), id, "start")?,
        episodes: asked(attrs.episode_count, id, "count")?,
        status: asked(attrs.status.clone(), id, "status")?,
        poster: poster.medium.or(poster.small).or(poster.original),
        cover: cover.large.or(cover.original),
    })
}

/// Parse one `media-relationships?include=destination` page of the
/// franchise read. A relation pointing nowhere is dropped; one whose
/// destination is not included, or an anime that leaves out a field it
/// was asked for, is no answer.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body is not that document.
pub fn parse_franchise_page(body: &[u8]) -> Result<FranchisePage> {
    let doc: Doc = serde_json::from_slice(body).map_err(|e| parse_error(e.to_string()))?;
    let read = doc.data.len();
    let mut relations = Vec::new();
    for edge in doc.data {
        let Some(dst) = edge.relationships.destination.data.into_inner() else {
            continue;
        };
        let node = doc
            .included
            .iter()
            .find(|n| n.kind == dst.kind && n.id == dst.id)
            .ok_or_else(|| parse_error(format!("{} {} not included", dst.kind, dst.id)))?;
        let entry = match dst.kind.as_str() {
            "anime" => Some(entry(&dst.id, &node.attributes)?),
            _ => None,
        };
        relations.push(FranchiseRelation {
            role: edge.attributes.role,
            subtype: node.attributes.subtype.clone(),
            kind: dst.kind,
            id: dst.id,
            entry,
        });
    }
    Ok(FranchisePage {
        relations,
        read,
        count: doc.meta.and_then(|m| m.count),
    })
}
