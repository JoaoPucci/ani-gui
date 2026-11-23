//! What one `/anime/:id?include=mappings` answer says about a Kitsu
//! anime: its external ids, and whether Kitsu has a cover for it. The
//! cover rides the same request at the cost of one field: a Continue
//! strip's warm reads every show's mappings before it asks AniList for
//! anything, and a show Kitsu has a cover for needs no AniList banner
//! (`commands::strip_warm`). Split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use serde::Deserialize;

use super::ExternalIds;
use crate::error::{AniError, Result};

/// One mappings answer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KitsuMappings {
    /// The external ids its mappings carry.
    pub ids: ExternalIds,
    /// Whether the anime carries a `coverImage`: what a detail read
    /// looks at before it asks AniList for a banner.
    pub has_cover: bool,
}

/// Read a Kitsu `/anime/:id?include=mappings` payload. Non-numeric
/// external ids and non-anime sites (`anilist/manga`, …) are ignored;
/// a `coverImage` that is absent or `null` is no cover.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't valid
/// JSON:API for an anime resource.
pub fn parse_mappings(body: &[u8]) -> Result<KitsuMappings> {
    #[derive(Deserialize)]
    struct Wrap {
        // The anime document itself: without it the reply is not the
        // one asked for, and its lack of mappings says nothing.
        data: AnimeDoc,
        #[serde(default)]
        included: Vec<Resource>,
    }
    #[derive(Deserialize)]
    struct AnimeDoc {
        #[serde(rename = "id")]
        _id: String,
        #[serde(default)]
        attributes: Option<AnimeAttrs>,
    }
    #[derive(Deserialize)]
    struct AnimeAttrs {
        #[serde(rename = "coverImage", default)]
        cover_image: Option<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Resource {
        #[serde(rename = "type")]
        type_: String,
        attributes: Option<Attrs>,
    }
    #[derive(Deserialize)]
    struct Attrs {
        #[serde(rename = "externalSite")]
        external_site: Option<String>,
        #[serde(rename = "externalId")]
        external_id: Option<String>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("kitsu mappings parse: {e}"),
    })?;
    let mut ids = ExternalIds::default();
    for r in parsed
        .included
        .into_iter()
        .filter(|r| r.type_ == "mappings")
    {
        let Some(attrs) = r.attributes else { continue };
        let parsed_id = attrs.external_id.and_then(|s| s.parse::<u32>().ok());
        match attrs.external_site.as_deref() {
            Some("myanimelist/anime") => ids.mal = ids.mal.or(parsed_id),
            Some("anilist/anime") => ids.anilist = ids.anilist.or(parsed_id),
            _ => {}
        }
    }
    let has_cover = parsed.data.attributes.and_then(|a| a.cover_image).is_some() && false;
    Ok(KitsuMappings { ids, has_cover })
}

#[cfg(test)]
#[path = "kitsu_mappings_test.rs"]
mod tests;
