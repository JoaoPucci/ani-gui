//! Reading AniList's `streamingEpisodes` reply into a
//! [`StreamingListing`]; split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use super::*;

/// Pure parser for the `streamingEpisodes` response body.
///
/// Entries with a null thumbnail or title are dropped, then entries
/// whose title doesn't yield an integer episode number via
/// [`split_episode_title`]. When several entries carry the same
/// number the first one wins — AniList lists the subbed and dubbed
/// tracks of an episode in that order.
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
        media: Option<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        #[serde(default)]
        episodes: Option<u32>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default, rename = "streamingEpisodes")]
        streaming_episodes: Option<Vec<StreamingEpisode>>,
    }
    #[derive(Deserialize)]
    struct StreamingEpisode {
        title: Option<String>,
        thumbnail: Option<String>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist streamingEpisodes response: {e}"),
    })?;
    let Some(media) = parsed.data.media else {
        return Ok(StreamingListing::default());
    };
    let mut listing = StreamingListing {
        episodes: media.episodes,
        finished: media.status.as_deref() == Some("FINISHED"),
        entries: BTreeMap::new(),
    };
    for e in media.streaming_episodes.unwrap_or_default() {
        let (Some(thumbnail), Some(title)) = (e.thumbnail, e.title) else {
            continue;
        };
        let Some((num, rest)) = split_episode_title(&title) else {
            continue;
        };
        listing.entries.entry(num).or_insert(StreamingEntry {
            title: rest.to_string(),
            thumbnail,
        });
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
