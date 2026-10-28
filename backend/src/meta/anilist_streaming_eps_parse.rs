//! Reading AniList's `streamingEpisodes` reply into a
//! [`StreamingListing`]; split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use super::*;

/// Pure parser for the `streamingEpisodes` response body.
///
/// Entries with a null title are dropped, then entries whose title
/// doesn't yield an integer episode number via
/// [`split_episode_title`]. An entry without a still is kept: it
/// still numbers and titles the episode for the fit rule, and fills
/// nothing. When several entries carry the same number the first
/// one's title wins — AniList lists the subbed and dubbed tracks of an
/// episode in that order — and the first still among them.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the
/// expected `{ data: { Media: { streamingEpisodes: [...] } } }`
/// envelope.
pub fn parse_streaming_episodes_response(body: &[u8]) -> Result<StreamingListing> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: crate::meta::present::Present<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        #[serde(default)]
        episodes: Option<u32>,
        #[serde(default)]
        status: Option<String>,
        #[serde(rename = "streamingEpisodes")]
        streaming_episodes: crate::meta::present::Present<Vec<StreamingEpisode>>,
    }
    #[derive(Deserialize)]
    struct StreamingEpisode {
        title: Option<String>,
        thumbnail: Option<String>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist streamingEpisodes response: {e}"),
    })?;
    let Some(media) = parsed.data.media.into_inner() else {
        return Ok(StreamingListing::default());
    };
    let mut listing = StreamingListing {
        episodes: media.episodes,
        finished: media.status.as_deref() == Some("FINISHED"),
        entries: BTreeMap::new(),
    };
    for e in media.streaming_episodes.into_inner().unwrap_or_default() {
        let Some(title) = e.title else { continue };
        let Some((num, rest)) = split_episode_title(&title) else {
            continue;
        };
        let entry = listing.entries.entry(num).or_insert(StreamingEntry {
            title: rest.to_string(),
            thumbnail: None,
        });
        if entry.thumbnail.is_none() {
            entry.thumbnail = e.thumbnail;
        }
    }
    Ok(listing)
}

/// Split an AniList streaming-episode title like `"Episode 1 - …"`
/// into its integer episode number and the title after it (separator
/// and surrounding whitespace removed). Returns `None` for titles
/// that don't start with `"Episode N"` (OVAs, movies, specials) and
/// for half-episode recap titles like `"Episode 1061.5 - …"` — Kitsu's
/// `KitsuEpisode::number` is `Option<u32>`, so half-eps can't merge.
///
/// Case-insensitive on the literal prefix. Whitespace-trimmed on the
/// input.
pub(super) fn split_episode_title(title: &str) -> Option<(u32, &str)> {
    let trimmed = title.trim();
    let rest = trimmed
        .strip_prefix("Episode ")
        .or_else(|| trimmed.strip_prefix("episode "))
        .or_else(|| trimmed.strip_prefix("EPISODE "))?;
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digits_end == 0 {
        return None;
    }
    let after = &rest[digits_end..];
    if after.starts_with('.') {
        return None;
    }
    let num = rest[..digits_end].parse::<u32>().ok()?;
    let words =
        after.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | '—' | ':'));
    Some((num, words.trim_end()))
}
