//! One page of a batched streaming-listing lookup: its parser, which
//! refuses a page that leaves out what the batch asked for, and reads
//! each node with the single lookup's own node parser. Split from
//! `anilist_streaming_eps_batch.rs` so each file stays inside the CRAP
//! gate's per-file bar.

use serde::Deserialize;

use super::super::parse::{listing_of, Media};
use super::{Keyed, Space};
use crate::error::{AniError, Result};
use crate::meta::anilist_pages::{by_key, Page};

/// The page a batch request came back as: cut, or whole with each
/// node's listing by the key it was asked by.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body is not a `Page` holding
/// `pageInfo.hasNextPage` and `media`, or a node is not an object or
/// leaves out a field the batch asked for. A node whose key is null
/// names no show and is left out; one whose listing cannot be read is
/// handed to its show's single lookup, with the keys several nodes
/// answer.
pub(crate) fn parse_streaming_page(body: &[u8], space: Space) -> Result<Page<Keyed>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Page")]
        page: PageShape,
    }
    #[derive(Deserialize)]
    struct PageShape {
        #[serde(rename = "pageInfo")]
        info: Info,
        media: Vec<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Info {
        #[serde(rename = "hasNextPage")]
        has_next: bool,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| failed(&e.to_string()))?;
    let page = parsed.data.page;
    if page.info.has_next {
        return Ok(Page::Cut);
    }
    let key_name = match space {
        Space::Mal => "idMal",
        Space::AniList => "id",
    };
    let mut media = Vec::with_capacity(page.media.len());
    let mut unreadable = Vec::new();
    for node in page.media {
        match node_of(node, key_name)? {
            Node::Read(key, listing) => media.push((key, listing)),
            Node::Unreadable(key) => unreadable.push(key),
            Node::Nameless => {}
        }
    }
    let (mut once, mut several) = by_key(media);
    for key in unreadable {
        once.remove(&key);
        if !several.contains(&key) {
            several.push(key);
        }
    }
    Ok(Page::Whole((once, several)))
}

/// What one node of a page reads as.
enum Node {
    Read(u32, crate::meta::anilist_streaming_eps::StreamingListing),
    /// A node naming its show whose listing cannot be read: the show
    /// goes to its single lookup, which reads that failure for it alone
    /// rather than failing every show on the page.
    Unreadable(u32),
    /// A node whose key is null names no show.
    Nameless,
}

/// One node's key and listing.
fn node_of(node: serde_json::Value, key_name: &str) -> Result<Node> {
    if !node.is_object() {
        return Err(failed("a node that is not an object"));
    }
    let asked = [key_name, "episodes", "status", "streamingEpisodes"];
    if let Some(missing) = asked.iter().find(|f| node.get(**f).is_none()) {
        return Err(failed(&format!("a node without {missing}")));
    }
    let key = match &node[key_name] {
        serde_json::Value::Null => return Ok(Node::Nameless),
        v => v
            .as_u64()
            .and_then(|k| u32::try_from(k).ok())
            .ok_or_else(|| failed(&format!("a node whose {key_name} is not an id")))?,
    };
    Ok(match serde_json::from_value::<Media>(node) {
        Ok(media) => Node::Read(key, listing_of(media)),
        Err(_) => Node::Unreadable(key),
    })
}

fn failed(detail: &str) -> AniError {
    AniError::ParseFailed {
        detail: format!("anilist streamingEpisodes batch response: {detail}"),
    }
}
