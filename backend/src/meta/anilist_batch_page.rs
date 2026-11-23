//! One page of a batched lookup: its parser, which refuses a page that
//! leaves out what the batch asked for, and reads each node with the
//! kind's own node reader. Split from `anilist_batch.rs` so each file
//! stays inside the CRAP gate's per-file bar.

use serde::Deserialize;

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
pub(crate) fn parse_page<T>(
    body: &[u8],
    space: Space,
    fields: &[&str],
    read: fn(serde_json::Value) -> Option<T>,
) -> Result<Page<Keyed<T>>> {
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
        match node_of(node, key_name, fields, read)? {
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
enum Node<T> {
    Read(u32, T),
    /// A node naming its show whose listing cannot be read: the show
    /// goes to its single lookup, which reads that failure for it alone
    /// rather than failing every show on the page.
    Unreadable(u32),
    /// A node whose key is null names no show.
    Nameless,
}

/// One node's key and listing.
fn node_of<T>(
    node: serde_json::Value,
    key_name: &str,
    fields: &[&str],
    read: fn(serde_json::Value) -> Option<T>,
) -> Result<Node<T>> {
    if !node.is_object() {
        return Err(failed("a node that is not an object"));
    }
    let asked = std::iter::once(&key_name).chain(fields);
    if let Some(missing) = asked.into_iter().find(|f| node.get(**f).is_none()) {
        return Err(failed(&format!("a node without {missing}")));
    }
    let key = match &node[key_name] {
        serde_json::Value::Null => return Ok(Node::Nameless),
        v => v
            .as_u64()
            .and_then(|k| u32::try_from(k).ok())
            .ok_or_else(|| failed(&format!("a node whose {key_name} is not an id")))?,
    };
    Ok(match read(node) {
        Some(read) => Node::Read(key, read),
        None => Node::Unreadable(key),
    })
}

fn failed(detail: &str) -> AniError {
    AniError::ParseFailed {
        detail: format!("anilist batch response: {detail}"),
    }
}
