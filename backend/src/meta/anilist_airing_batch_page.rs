//! One page of an airing batch: its parser, which refuses a page that
//! leaves out what the batch asked for, and the partition of its nodes
//! by key. Split from `anilist_airing_batch.rs` so each file stays
//! inside the CRAP gate's per-file bar.

use serde::Deserialize;

use super::super::{derive_status, AiringStatus, MediaShape};
use crate::error::{AniError, Result};

/// One page of a batch answer: each media node's key (`idMal` or `id`)
/// and status, and whether AniList cut the page.
pub(super) struct BatchPage {
    pub(super) media: Vec<(u32, AiringStatus)>,
    pub(super) cut: bool,
}

impl BatchPage {
    /// The keys one node answers, with their status, and the keys
    /// several nodes answer, which only a single lookup can settle.
    pub(super) fn by_key(self) -> (std::collections::HashMap<u32, AiringStatus>, Vec<u32>) {
        let mut once = std::collections::HashMap::new();
        let mut several = Vec::new();
        for (key, status) in self.media {
            if several.contains(&key) {
                continue;
            }
            if once.insert(key, status).is_some() {
                once.remove(&key);
                several.push(key);
            }
        }
        (once, several)
    }
}

/// Pure parser for the batch response: every returned media node
/// with an `id` becomes a map entry via [`derive_status`].
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope.
pub fn parse_airing_batch_response(
    body: &[u8],
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    Ok(parse_batch(body, false)?.media.into_iter().collect())
}

/// Batch parser keyed by `idMal` (`by_mal`) or `id`, in page order.
pub(super) fn parse_batch(body: &[u8], by_mal: bool) -> Result<BatchPage> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Page")]
        page: Page,
    }
    // Every field here is one the batch asks for: a page that leaves one
    // out is no answer, never an empty or whole one.
    #[derive(Deserialize)]
    struct Page {
        #[serde(rename = "pageInfo")]
        info: Info,
        media: Vec<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Info {
        #[serde(rename = "hasNextPage")]
        has_next: bool,
    }
    let failed = |detail: String| AniError::ParseFailed {
        detail: format!("anilist airing batch response: {detail}"),
    };
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| failed(e.to_string()))?;
    let page = parsed.data.page;
    let key_name = if by_mal { "idMal" } else { "id" };
    let mut media = Vec::with_capacity(page.media.len());
    for node in page.media {
        // A node that leaves out a field it was asked for cannot say whose
        // schedule it is, or how far the show has aired; one whose key is
        // null names no show.
        if !node.is_object() {
            return Err(failed("a node that is not an object".into()));
        }
        let asked = [
            key_name,
            "status",
            "episodes",
            "nextAiringEpisode",
            "airingSchedule",
        ];
        if let Some(missing) = asked.iter().find(|f| node.get(**f).is_none()) {
            return Err(failed(format!("a node without {missing}")));
        }
        let schedule = node.get("airingSchedule").filter(|v| !v.is_null());
        if schedule.is_some_and(|v| v.get("nodes").is_none()) {
            return Err(failed("a schedule without its nodes".into()));
        }
        let m: MediaShape = serde_json::from_value(node).map_err(|e| failed(e.to_string()))?;
        let key = if by_mal { m.id_mal } else { m.id };
        if let Some(k) = key {
            media.push((k, derive_status(m)));
        }
    }
    Ok(BatchPage {
        cut: page.info.has_next,
        media,
    })
}
