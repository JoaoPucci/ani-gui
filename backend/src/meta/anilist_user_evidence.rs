//! AniList's answer to what the episode map asks of an entry; split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use super::*;
use crate::account::episode_map::TrackerShow;

/// GraphQL: the media as the episode map reads it — total, status,
/// run, relations (type and format decide which are series sequels)
/// and the titles of its streaming episodes, whose numbers identify a
/// prologue or a recap the entry counts.
pub(super) const MEDIA_EVIDENCE_GQL: &str = "query Evidence($mediaId: Int!) { \
        Media(id: $mediaId) { \
            id idMal episodes status format \
            startDate { year month day } endDate { year month day } \
            relations { edges { relationType node { id type format } } } \
            streamingEpisodes { title } \
        } \
    }";

/// AniList's fuzzy date as `YYYY-MM-DD`, or as much of it as is known
/// (`YYYY-MM`, `YYYY`); `None` without a year, or where the month or day
/// names no real calendar date — an impossible date is no date, never
/// one carried into a neighbouring month.
fn fuzzy_date(year: Option<u32>, month: Option<u32>, day: Option<u32>) -> Option<String> {
    let year = format!("{:04}", year?);
    let Some(month) = month else {
        return (year.len() == 4).then_some(year);
    };
    let out = match day {
        Some(day) => format!("{year}-{month:02}-{day:02}"),
        None => format!("{year}-{month:02}"),
    };
    crate::account::episode_map_calendar::parse_date(&out).map(|_| out)
}

/// The number a streaming-episode title carries — `"Episode 6.5 -
/// Ekwari"` is `"6.5"`, `"Episode 0 - Prologue"` is `"0"` — or `None`
/// for a title that does not open on `Episode <number>`.
fn listed_number_of(title: &str) -> Option<String> {
    let trimmed = title.trim();
    let rest = trimmed
        .strip_prefix("Episode ")
        .or_else(|| trimmed.strip_prefix("episode "))?;
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!number.is_empty() && number.starts_with(|c: char| c.is_ascii_digit())).then_some(number)
}

/// A field the query asks for, read as given: `None` when it is null.
/// Named by `deserialize_with`, which leaves a missing field the
/// missing-field error where a bare `Option` would read it as `None`.
fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// Whether a format is a series': a movie or a music video is not.
fn is_series(format: Option<&str>) -> bool {
    !matches!(format, Some("MOVIE" | "MUSIC"))
}

/// One relation edge, every key the query asks for required.
#[derive(Deserialize)]
struct Edge {
    #[serde(rename = "relationType", deserialize_with = "present")]
    relation_type: Option<String>,
    #[serde(deserialize_with = "present")]
    node: Option<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: u32,
    #[serde(rename = "type", deserialize_with = "present")]
    kind: Option<String>,
    #[serde(deserialize_with = "present")]
    format: Option<String>,
}

/// The anime series among an entry's sequel edges. A sequel edge whose
/// node is null names a sequel the reply does not identify, which
/// leaves the sequels unknown: no answer, never none.
fn series_sequels(edges: Vec<Edge>) -> Result<Vec<ProviderMediaId>> {
    let mut sequels = Vec::new();
    for edge in edges
        .into_iter()
        .filter(|e| e.relation_type.as_deref() == Some("SEQUEL"))
    {
        let node = edge.node.ok_or_else(|| AniError::ParseFailed {
            detail: "anilist media evidence: a sequel with no node".into(),
        })?;
        if node.kind.as_deref() == Some("ANIME") && is_series(node.format.as_deref()) {
            sequels.push(ProviderMediaId(node.id));
        }
    }
    Ok(sequels)
}

/// Pure parser for the [`MEDIA_EVIDENCE_GQL`] response. The query asks
/// for `relations { edges { relationType node { id type format } } }`:
/// a reply that leaves out any of those keys, gives `relations` or
/// `edges` as null, or names a sequel by a null node does not say what
/// the entry's sequels are, so it is no answer. A null relation type,
/// or a null node on an edge that is no sequel, says no sequel there.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the response isn't the
/// `{ data: { Media: { … } } }` envelope with a media in it, or its
/// relations do not say what the sequels are.
pub(super) fn parse_media_evidence_response(body: &[u8]) -> Result<MediaEvidence> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: Option<Media>,
    }
    #[derive(Deserialize, Default)]
    struct FuzzyDate {
        year: Option<u32>,
        month: Option<u32>,
        day: Option<u32>,
    }
    #[derive(Deserialize)]
    struct Media {
        id: u32,
        #[serde(rename = "idMal", deserialize_with = "present")]
        id_mal: Option<u32>,
        #[serde(deserialize_with = "present")]
        episodes: Option<u32>,
        #[serde(deserialize_with = "present")]
        status: Option<String>,
        #[serde(deserialize_with = "present")]
        format: Option<String>,
        #[serde(rename = "startDate", default)]
        start_date: Option<FuzzyDate>,
        #[serde(rename = "endDate", default)]
        end_date: Option<FuzzyDate>,
        relations: Relations,
        #[serde(rename = "streamingEpisodes", default)]
        streaming_episodes: Option<Vec<StreamingEpisode>>,
    }
    #[derive(Deserialize)]
    struct Relations {
        edges: Vec<Edge>,
    }
    #[derive(Deserialize)]
    struct StreamingEpisode {
        title: Option<String>,
    }
    let wrap: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist media evidence: {e}"),
    })?;
    let media = wrap.data.media.ok_or_else(|| AniError::ParseFailed {
        detail: "anilist media evidence: no Media".into(),
    })?;
    let date = |d: Option<FuzzyDate>| {
        let d = d.unwrap_or_default();
        fuzzy_date(d.year, d.month, d.day)
    };
    let sequels = series_sequels(media.relations.edges)?;
    let listed = media
        .streaming_episodes
        .filter(|eps| !eps.is_empty())
        .map(|eps| {
            eps.into_iter()
                .filter_map(|e| listed_number_of(e.title.as_deref()?))
                .collect()
        });
    Ok(MediaEvidence {
        show: TrackerShow {
            id: media.id,
            id_mal: media.id_mal,
            episodes: media.episodes.filter(|n| *n > 0),
            finished: media.status.as_deref() == Some("FINISHED"),
            start: date(media.start_date),
            end: date(media.end_date),
        },
        series: is_series(media.format.as_deref()),
        sequels,
        listed,
    })
}

#[cfg(test)]
#[path = "anilist_user_evidence_prop_test.rs"]
mod prop_tests;
