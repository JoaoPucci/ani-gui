//! Every Kitsu entry mapped to one tracker id. Kitsu's
//! `/mappings?filter[externalSite]=…&filter[externalId]=…` answers with
//! all of them, each inlined; the lookups behind a card take the first
//! ([`super::KitsuClient::lookup_by_mal_id`]), and the episode map reads
//! them all. Split from [`super`] so each file stays inside the CRAP
//! gate's per-file bar.

use super::*;

/// A side-loaded resource of any type; only `anime` ones are read.
#[derive(Deserialize)]
struct Included {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    attributes: serde_json::Value,
}

/// The inlined anime among `included`, by id.
fn inlined_anime(included: Vec<Included>) -> HashMap<String, KitsuAnimeRef> {
    included
        .into_iter()
        .filter(|r| r.kind == "anime")
        .filter_map(|r| {
            let attributes: AnimeAttributes = serde_json::from_value(r.attributes).ok()?;
            Some((
                r.id.clone(),
                into_ref(AnimeResource {
                    id: r.id,
                    attributes,
                }),
            ))
        })
        .collect()
}

/// A JSON:API relationship pointer: `{ data: { type, id } }`, `data`
/// null where there is nothing on the other end.
#[derive(Deserialize)]
struct Pointer {
    #[serde(default)]
    data: Option<PointerRef>,
}

#[derive(Deserialize)]
struct PointerRef {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

/// The Kitsu entries mapped to one tracker id, and whether the page
/// read held all of them.
#[derive(Debug, Clone, PartialEq)]
pub struct KitsuMapped {
    /// The entries on the page read, in Kitsu's order.
    pub entries: Vec<KitsuAnimeRef>,
    /// Whether they are every mapping Kitsu counts.
    pub complete: bool,
}

/// Every anime a `/mappings?filter[externalSite]=…&filter[externalId]=…
/// &include=item` body inlines for its mappings, in Kitsu's order, a
/// music-subtype entry dropped as every mapping lookup drops it.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't JSON:API.
pub fn parse_mapped_anime_response(body: &[u8]) -> Result<KitsuMapped> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Vec<Mapping>,
        #[serde(default)]
        included: Vec<Included>,
    }
    #[derive(Deserialize)]
    struct Mapping {
        #[serde(default)]
        relationships: Option<MappingRelationships>,
    }
    #[derive(Deserialize)]
    struct MappingRelationships {
        #[serde(default)]
        item: Option<Pointer>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("kitsu mapped anime parse: {e}"),
    })?;
    let mut anime = inlined_anime(parsed.included);
    let entries = parsed
        .data
        .into_iter()
        .filter_map(|m| m.relationships?.item?.data)
        .filter(|r| r.kind == "anime")
        .filter_map(|r| anime.remove(&r.id))
        .filter(|a| !is_music_subtype(a.subtype.as_deref()))
        .collect();
    Ok(KitsuMapped {
        entries,
        complete: true,
    })
}

impl KitsuClient {
    /// A GET against `path` with the JSON:API accept header, answering
    /// the body on 2xx.
    async fn get_json_api(&self, path: &str, query: &[(&str, String)]) -> Result<bytes::Bytes> {
        let resp = self
            .http
            .get(format!("{}/{path}", self.base))
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .query(query)
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        if !resp.status().is_success() {
            return Err(AniError::Upstream {
                status: resp.status().as_u16(),
            });
        }
        resp.bytes().await.map_err(|_| AniError::Network)
    }

    /// Every Kitsu anime mapped to `external_id` on `site`
    /// (`myanimelist/anime`, `anilist/anime`), each inlined in full.
    /// Several Kitsu entries can map to one tracker entry — the Attack
    /// on Titan Final Chapters specials both map to one MyAnimeList id
    /// — and the episode map reads them all; [`Self::lookup_by_mal_id`]
    /// takes the first for a card.
    ///
    /// # Errors
    /// Same as [`Self::search`].
    pub async fn anime_by_external_id(&self, site: &str, external_id: u32) -> Result<KitsuMapped> {
        let body = self
            .get_json_api(
                "mappings",
                &[
                    ("filter[externalSite]", site.to_string()),
                    ("filter[externalId]", external_id.to_string()),
                    ("include", "item".to_string()),
                    ("fields[anime]", ANIME_FIELDS.to_string()),
                ],
            )
            .await?;
        parse_mapped_anime_response(&body)
    }
}

#[cfg(test)]
#[path = "kitsu_mapped_test.rs"]
mod tests;
