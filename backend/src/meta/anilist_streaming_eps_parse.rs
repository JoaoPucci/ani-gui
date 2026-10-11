//! Reading AniList's `streamingEpisodes` reply into episode numbers and
//! thumbnails; split from [`super`] so each file stays inside the CRAP
//! gate's per-file bar.

use super::*;

/// Pure parser for the `streamingEpisodes` response body.
///
/// Filters in one pass: entries with a null thumbnail are dropped,
/// then entries whose title doesn't yield an integer episode number
/// via [`extract_integer_episode_number`] are dropped. Order is
/// preserved — callers downstream pick first-wins on collisions.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the
/// expected `{ data: { Media: { streamingEpisodes: [...] } } }`
/// envelope.
pub fn parse_streaming_episodes_response(body: &[u8]) -> Result<Vec<(u32, String)>> {
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
    let eps = parsed
        .data
        .media
        .and_then(|m| m.streaming_episodes)
        .unwrap_or_default();
    let mut out = Vec::with_capacity(eps.len());
    for e in eps {
        let Some(thumb) = e.thumbnail else { continue };
        let Some(title) = e.title.as_deref() else {
            continue;
        };
        let Some(num) = extract_integer_episode_number(title) else {
            continue;
        };
        out.push((num, thumb));
    }
    Ok(out)
}

/// Extract the integer episode number from an AniList streaming-
/// episode title like `"Episode 1 - …"`. Returns `None` for titles
/// that don't start with `"Episode N"` (OVAs, movies, specials) and
/// for half-episode recap titles like `"Episode 1061.5 - …"` — Kitsu's
/// `KitsuEpisode::number` is `Option<u32>`, so half-eps can't merge.
///
/// Case-insensitive on the literal prefix. Whitespace-trimmed on the
/// input.
fn extract_integer_episode_number(title: &str) -> Option<u32> {
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
    let after = rest[digits_end..].chars().next();
    if after == Some('.') {
        return None;
    }
    rest[..digits_end].parse::<u32>().ok()
}
