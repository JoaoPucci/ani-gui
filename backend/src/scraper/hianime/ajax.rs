//! The JSON envelopes hianime's AJAX endpoints answer — `{"status":
//! true, "html": "…"}` — and the HTML each one wraps: the episode
//! list and the per-episode server list.

use base64::Engine as _;
use serde::Deserialize;

use crate::error::{AniError, Result};
use crate::scraper::provider::EpisodeRef;

#[derive(Deserialize)]
struct Envelope {
    status: bool,
    #[serde(default)]
    html: Option<String>,
}

/// The HTML an envelope wraps. `status: false` is the provider
/// answering not-found for the id asked about — the same shape a
/// dead slug's 404 carries elsewhere — so it surfaces as one.
///
/// # Errors
/// [`AniError::Upstream`] with 404 on `status: false`,
/// [`AniError::ParseFailed`] when the body is not the envelope.
fn unwrap_envelope(json: &str) -> Result<String> {
    let env: Envelope = serde_json::from_str(json).map_err(|e| AniError::ParseFailed {
        detail: format!("hianime envelope: {e}"),
    })?;
    if !env.status {
        return Err(AniError::Upstream { status: 404 });
    }
    env.html.ok_or_else(|| AniError::ParseFailed {
        detail: "hianime envelope without html".into(),
    })
}

/// An empty listing is the provider answering "none"; nonempty HTML
/// that yields no usable row is the site having changed shape —
/// the marker gone, its attributes renamed, every hash in a format
/// the parser does not open — and reading that as "none" would let
/// the mode probe persist an absence that hides a playable show. A
/// listing with at least one usable row skips the rest. The episode
/// list has one more shape that means "none": its container rendered
/// with nothing inside ([`is_blank_listing`]).
///
/// # Errors
/// [`AniError::ParseFailed`] for nonempty HTML that produced no row.
fn recognized<T>(html: &str, rows: Vec<T>, what: &str) -> Result<Vec<T>> {
    if rows.is_empty() && !html.trim().is_empty() {
        return Err(AniError::ParseFailed {
            detail: format!("hianime {what} without recognizable rows"),
        });
    }
    Ok(rows)
}

/// The value of the first `name="…"` attribute in `s`.
fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let (_, rest) = s.split_once(name)?;
    rest.split('"').next()
}

/// Each open tag carrying `marker`, whole — from its `<` to its
/// `>` — in page order. A row's attributes are read from its own
/// tag rather than from the text after the marker, so the site may
/// write them in any order around the marker class and the row
/// still reads as itself; taking the text after the marker as the
/// row would find a marker-last row's attributes in the chunk before
/// it, and read each row's attributes as the next row's. A marker
/// that is not inside a tag is passed over, and a tag carrying the
/// marker twice is one tag.
fn marked_tags<'a>(html: &'a str, marker: &str) -> Vec<&'a str> {
    let mut tags = Vec::new();
    let mut from = 0;
    while let Some(found) = html[from..].find(marker) {
        let at = from + found;
        from = at + marker.len();
        let Some(open) = html[..at].rfind('<') else {
            continue;
        };
        if html[open..at].contains('>') {
            continue;
        }
        let Some(close) = html[at..].find('>') else {
            break;
        };
        tags.push(&html[open..=at + close]);
        from = at + close + 1;
    }
    tags
}

/// Whether `html` is the episode list's container with nothing but
/// whitespace inside — how the site renders an entry it has announced
/// but not started serving. That is the provider answering "no
/// episodes"; the same container holding anything the parser does
/// not read is a changed shape, and stays one.
fn is_blank_listing(html: &str) -> bool {
    html.split_once("class=\"ss-list\"")
        .and_then(|(_, rest)| rest.split_once('>'))
        .and_then(|(_, body)| body.split_once("</div>"))
        .is_some_and(|(inside, _)| inside.trim().is_empty())
}

/// An entry's episodes: each `ep-item` anchor's `data-number` and
/// `data-id`, read from the anchor's own tag ([`marked_tags`]) so the
/// order the site writes its attributes in does not matter. A row's
/// slot is its position in the listing — what the history and a
/// resume key on — and the site's number is its display tag
/// whenever it is not that position: the site numbers a recap or a
/// special `7.5`, and the rows after it then say `8` at the ninth
/// position, exactly as anidb.app's rows carry theirs. The resolver
/// matches the tag verbatim, so nothing about the number is
/// decided here. A row without a number or without an id is one the
/// reader cannot read, and it refuses the listing whole: the
/// listing is the show's count, and one short of it undercounts the
/// show and loses the dropped row's link. A listing whose container
/// holds nothing is the entry having no episodes yet.
///
/// # Errors
/// As [`unwrap_envelope`]; [`AniError::ParseFailed`] naming the row
/// when a marked row cannot be read.
pub fn parse_episode_list(json: &str) -> Result<Vec<EpisodeRef>> {
    let html = unwrap_envelope(json)?;
    let rows = marked_tags(&html, "ep-item")
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            episode_row(index, item).ok_or_else(|| AniError::ParseFailed {
                detail: format!("hianime episode list: row {} cannot be read", index + 1),
            })
        })
        .collect::<Result<Vec<EpisodeRef>>>()?;
    if rows.is_empty() && is_blank_listing(&html) {
        return Ok(rows);
    }
    recognized(&html, rows, "episode list")
}

/// One marked row of the episode listing, or nothing when its
/// number is blank or its id is missing or not a number.
fn episode_row(index: usize, item: &str) -> Option<EpisodeRef> {
    let number = u32::try_from(index + 1).ok()?;
    let tag = attr(item, "data-number=\"")?.trim();
    if tag.is_empty() {
        return None;
    }
    let id = attr(item, "data-id=\"")?.trim().parse().ok()?;
    Some(EpisodeRef {
        id,
        number,
        number2: (tag != number.to_string()).then(|| tag.to_string()),
    })
}

/// One playable server for an episode: its audio mode, the site's
/// name for it, and the embed URL its hash decodes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEmbed {
    /// `sub` or `dub`, as the site types the server.
    pub mode: String,
    /// The site's server name (`HD-1`, `HD-2`).
    pub name: String,
    /// The player page, on the embed host.
    pub embed_url: String,
}

/// The modes the site types its servers with, and the two the mode
/// probe asks for.
const KNOWN_MODES: [&str; 2] = ["sub", "dub"];

/// An episode's server listing as read: the servers whose rows the
/// client could use, the modes that had a row the client could not —
/// a hash that does not decode, a decoded value that is not an
/// absolute http(s) URL, a name missing — and whether a row was typed
/// with a mode the client does not know. Both doubts are kept because
/// a listing can be readable for one mode and not the other, and "no
/// readable sub server" read as "no sub" is an absence the mode probe
/// persists over a playback the site still lists — whether the sub
/// row's hash stopped decoding or the site renamed `sub` to something
/// else while `dub` stayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerListing {
    /// The servers the client could read, in the site's order.
    pub servers: Vec<ServerEmbed>,
    /// The known modes with at least one row the client could not
    /// read, whether or not another row of the mode could be; each
    /// mode at most once, in the order first seen.
    pub unreadable_modes: Vec<String>,
    /// Whether a row was seen whose mode the client cannot tell —
    /// typed blank, typed with a value other than `sub` and `dub`, or
    /// not typed at all. Such a row may be a known mode under a new
    /// name or attribute, so no known mode the listing carries no row
    /// of is absent while one was seen.
    pub unknown_modes: bool,
}

impl ServerListing {
    /// Whether `mode` is served: `true` with a readable server of it,
    /// `false` when the listing carried no row of it and every row
    /// was typed and read.
    ///
    /// # Errors
    /// [`AniError::ParseFailed`] when the mode has no readable server
    /// but is uncertain ([`Self::uncertain_for`]) — the shape changed
    /// under one mode, which is not the site listing none.
    pub fn mode_readable(&self, mode: &str) -> Result<bool> {
        if self.servers.iter().any(|s| s.mode == mode) {
            return Ok(true);
        }
        if self.uncertain_for(mode) {
            return Err(AniError::ParseFailed {
                detail: format!("hianime {mode} servers without a readable row"),
            });
        }
        Ok(false)
    }

    /// Whether the listing leaves `mode` in doubt: a row of the mode
    /// the client could not read, or a row typed with a mode it does
    /// not know — either may have been the mode's server.
    #[must_use]
    pub fn uncertain_for(&self, mode: &str) -> bool {
        self.unknown_modes || self.unreadable_modes.iter().any(|m| m == mode)
    }
}

/// One row of the server list, read as far as the client can.
enum ServerRow {
    /// A row the client read.
    Read(ServerEmbed),
    /// A row of a known mode the client could not read.
    Unreadable(String),
    /// A row whose mode the client cannot tell: typed with a mode it
    /// does not know, or not typed at all.
    Unknown,
}

/// One row of the server list, read as far as the client can. A row
/// without the mode attribute is a row whose mode the client cannot
/// tell, not a fragment to drop: the site's row marker is on it, and
/// the attribute renamed under the sub row would otherwise read as
/// the listing carrying no sub.
fn read_server_row(item: &str) -> ServerRow {
    let Some(mode) = attr(item, "data-type=\"").map(|m| m.trim().to_string()) else {
        return ServerRow::Unknown;
    };
    if !KNOWN_MODES.contains(&mode.as_str()) {
        return ServerRow::Unknown;
    }
    let read = || -> Option<ServerEmbed> {
        let name = attr(item, "data-server-name=\"")?.trim().to_string();
        let hash = attr(item, "data-hash=\"")?.trim();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(hash)
            .ok()?;
        let embed_url = String::from_utf8(bytes).ok()?;
        let parsed = url::Url::parse(&embed_url).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        Some(ServerEmbed {
            mode: mode.clone(),
            name,
            embed_url,
        })
    };
    read().map_or(ServerRow::Unreadable(mode), ServerRow::Read)
}

/// An episode's servers, with the listing's uncertainty per mode
/// ([`ServerListing`]). A row whose mode is not `sub` or `dub` — typed
/// as nothing, as some renamed value, or not typed at all — is not a
/// server of any mode the client knows and is skipped, but marks the
/// listing as carrying unknown modes: counting it as read would let a
/// listing of such rows pass as "no sub, no dub" instead of a changed
/// shape, and forgetting it would let a renamed mode or attribute
/// read as absent beside the mode that kept its name. A row of a known mode the client cannot
/// read marks that mode unreadable and is skipped; a nonempty listing
/// with no readable row at all is refused.
///
/// # Errors
/// As [`unwrap_envelope`], and [`AniError::ParseFailed`] for a
/// nonempty listing that produced no readable row.
pub fn parse_server_listing(json: &str) -> Result<ServerListing> {
    let html = unwrap_envelope(json)?;
    let mut servers = Vec::new();
    let mut unreadable_modes: Vec<String> = Vec::new();
    let mut unknown_modes = false;
    for row in marked_tags(&html, "server-item")
        .into_iter()
        .map(read_server_row)
    {
        match row {
            ServerRow::Read(server) => servers.push(server),
            ServerRow::Unreadable(mode) => {
                if !unreadable_modes.contains(&mode) {
                    unreadable_modes.push(mode);
                }
            }
            ServerRow::Unknown => unknown_modes = true,
        }
    }
    let servers = recognized(&html, servers, "server list")?;
    Ok(ServerListing {
        servers,
        unreadable_modes,
        unknown_modes,
    })
}

/// An episode's servers alone — [`parse_server_listing`] without the
/// per-mode uncertainty, for callers that only walk the servers.
///
/// # Errors
/// As [`parse_server_listing`].
pub fn parse_servers(json: &str) -> Result<Vec<ServerEmbed>> {
    parse_server_listing(json).map(|listing| listing.servers)
}

/// The embed hosts whose pages carry the payload the client reads —
/// the `window.__P` blob. The site names its servers by slot and
/// moves the slots between hosts; a name is not a shape.
const READABLE_HOSTS: &[&str] = &["zokoanime.video"];

/// Whether `embed_url` is on a host whose page the client can read.
#[must_use]
pub fn readable(embed_url: &str) -> bool {
    url::Url::parse(embed_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .is_some_and(|h| READABLE_HOSTS.contains(&h.as_str()))
}

/// The servers to try for `mode`, in order: every server of that
/// mode, the ones on a host the client can read first, the site's
/// own order kept within each half. Empty when the mode has none.
#[must_use]
pub fn servers_for<'a>(servers: &'a [ServerEmbed], mode: &str) -> Vec<&'a ServerEmbed> {
    let of_mode = servers.iter().filter(|s| s.mode == mode);
    let (readable_hosts, rest): (Vec<_>, Vec<_>) = of_mode.partition(|s| readable(&s.embed_url));
    readable_hosts.into_iter().chain(rest).collect()
}
