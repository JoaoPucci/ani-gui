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
/// (`YYYY-MM`, `YYYY`); `None` without a year.
fn fuzzy_date(year: Option<u32>, month: Option<u32>, day: Option<u32>) -> Option<String> {
    let mut out = format!("{:04}", year?);
    if let Some(month) = month {
        out.push_str(&format!("-{month:02}"));
        if let Some(day) = day {
            out.push_str(&format!("-{day:02}"));
        }
    }
    Some(out)
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

/// Pure parser for the [`MEDIA_EVIDENCE_GQL`] response.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the response isn't the
/// `{ data: { Media: { … } } }` envelope with a media in it.
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
        #[serde(rename = "idMal")]
        id_mal: Option<u32>,
        episodes: Option<u32>,
        status: Option<String>,
        format: Option<String>,
        #[serde(rename = "startDate", default)]
        start_date: Option<FuzzyDate>,
        #[serde(rename = "endDate", default)]
        end_date: Option<FuzzyDate>,
        #[serde(default)]
        relations: Option<Relations>,
        #[serde(rename = "streamingEpisodes", default)]
        streaming_episodes: Option<Vec<StreamingEpisode>>,
    }
    #[derive(Deserialize)]
    struct Relations {
        #[serde(default)]
        edges: Vec<Edge>,
    }
    #[derive(Deserialize)]
    struct Edge {
        #[serde(rename = "relationType")]
        relation_type: Option<String>,
        node: Option<Node>,
    }
    #[derive(Deserialize)]
    struct Node {
        id: u32,
        #[serde(rename = "type")]
        kind: Option<String>,
        format: Option<String>,
    }
    #[derive(Deserialize)]
    struct StreamingEpisode {
        title: Option<String>,
    }
    let is_series = |format: Option<&str>| !matches!(format, Some("MOVIE" | "MUSIC"));
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
    let sequels = media
        .relations
        .map(|r| r.edges)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.relation_type.as_deref() == Some("SEQUEL"))
        .filter_map(|e| e.node)
        .filter(|n| n.kind.as_deref() == Some("ANIME") && is_series(n.format.as_deref()))
        .map(|n| ProviderMediaId(n.id))
        .collect();
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
