//! The relations read the franchise walk works from: every entry Kitsu
//! relates an anime to, with what a watch-order strip shows of each
//! related anime (title, format, start, count, status, artwork), paged
//! past Kitsu's twenty per page.
//!
//! Separate from [`super::relations`] (`media_relationships`), which
//! asks one page with no titles or artwork and feeds the tracker map;
//! the two are cached under families of their own.

use serde::{Deserialize, Serialize};

use super::KitsuClient;
use crate::error::{AniError, Result};
use crate::meta::present::{present, Present};

/// What the franchise surfaces show of one anime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FranchiseEntry {
    /// Its Kitsu id.
    pub id: String,
    /// Kitsu's canonical title.
    pub title: String,
    /// Its subtype as Kitsu serves it: `TV`, `movie`, `OVA`, ….
    pub subtype: Option<String>,
    /// The day it started, `YYYY-MM-DD`.
    pub start: Option<String>,
    /// Its episode count.
    pub episodes: Option<u32>,
    /// Its status: `finished`, `current`, ….
    pub status: Option<String>,
    /// Its poster: the medium size, else small, else the original.
    pub poster: Option<String>,
    /// Its cover: the large size, else the original.
    pub cover: Option<String>,
}

/// One relation of an entry's listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FranchiseRelation {
    /// How Kitsu relates it: `sequel`, `side_story`, `summary`, ….
    pub role: String,
    /// The related resource's type: `anime` or `manga`.
    pub kind: String,
    /// Its Kitsu id.
    pub id: String,
    /// Its subtype.
    pub subtype: Option<String>,
    /// What the surfaces show of it, for an anime.
    pub entry: Option<FranchiseEntry>,
}

/// Every relation Kitsu lists for an entry, as far as the pages read
/// reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FranchiseListing {
    /// The relations, in Kitsu's order across the pages read.
    pub relations: Vec<FranchiseRelation>,
    /// Whether they are every relation Kitsu counts: false when the
    /// page cap cut the listing short.
    pub complete: bool,
}

/// One page of a listing: its relations, how many rows it held (a
/// relation pointing nowhere included) and how many Kitsu counts in
/// all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FranchisePage {
    /// The relations that point somewhere.
    pub relations: Vec<FranchiseRelation>,
    /// The rows the page held.
    pub read: usize,
    /// Kitsu's `meta.count`.
    pub count: Option<usize>,
}

/// Relations asked per page — the most Kitsu serves at once.
pub const FRANCHISE_PAGE_LIMIT: usize = 20;
/// Pages read at most per listing. A listing longer than this is read
/// as incomplete.
pub const FRANCHISE_MAX_PAGES: usize = 5;
/// The fields asked of each related anime.
pub const FRANCHISE_ANIME_FIELDS: &str =
    "canonicalTitle,subtype,status,episodeCount,startDate,posterImage,coverImage";

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

/// Whether another page is worth asking for, after `pages` pages that
/// held `rows` rows in all, the last of them `last_read`, with Kitsu
/// counting `count`.
#[must_use]
pub fn more_pages(pages: usize, rows: usize, last_read: usize, count: Option<usize>) -> bool {
    pages < FRANCHISE_MAX_PAGES && last_read > 0 && count.is_some_and(|c| rows < c)
}

/// Whether `rows` rows read are every relation Kitsu counts.
#[must_use]
pub fn listing_complete(rows: usize, count: Option<usize>) -> bool {
    count.is_some_and(|c| rows >= c)
}

impl KitsuClient {
    /// Every relation Kitsu lists for `anime_id`, with each related
    /// anime's title, format, start, count, status and artwork, paged
    /// while Kitsu counts more, up to [`FRANCHISE_MAX_PAGES`].
    ///
    /// # Errors
    /// [`AniError::Network`] when Kitsu cannot be reached,
    /// [`AniError::Upstream`] for a non-success status, and the
    /// parser's errors.
    pub async fn franchise_relations(&self, anime_id: &str) -> Result<FranchiseListing> {
        let mut relations = Vec::new();
        let mut rows = 0;
        let mut pages = 0;
        loop {
            let page = self.franchise_page(anime_id, rows).await?;
            pages += 1;
            rows += page.read;
            relations.extend(page.relations);
            if !more_pages(pages, rows, page.read, page.count) {
                return Ok(FranchiseListing {
                    relations,
                    complete: listing_complete(rows, page.count),
                });
            }
        }
    }

    async fn franchise_page(&self, anime_id: &str, offset: usize) -> Result<FranchisePage> {
        let resp = self
            .http
            .get(format!(
                "{}/anime/{}/media-relationships",
                self.base, anime_id
            ))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(&[
                ("include", "destination".to_string()),
                ("fields[anime]", FRANCHISE_ANIME_FIELDS.to_string()),
                ("fields[manga]", "subtype".to_string()),
                ("page[limit]", FRANCHISE_PAGE_LIMIT.to_string()),
                ("page[offset]", offset.to_string()),
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
        parse_franchise_page(&body)
    }
}

#[cfg(test)]
#[path = "kitsu_franchise_read_test.rs"]
mod tests;

#[cfg(test)]
#[path = "kitsu_franchise_read_prop_test.rs"]
mod prop_tests;
