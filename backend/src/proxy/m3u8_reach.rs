//! Where a URI a manifest names leaves the rewrite: on the proxy, as
//! written, or not at all.
//!
//! The player fetches whatever the rewritten manifest names. A network
//! URI goes through the proxy, which carries the session's referer and
//! keeps the host's budget. A URI that is not a network one — anything
//! but `http` and `https` — is no fetch for the proxy, and what happens
//! to it depends on what the manifest names it as:
//!
//! - Media — a key, an init segment, a media segment, a master's
//!   session key or session data — stays as written. An inline `data:`
//!   key or init segment is bytes the player decodes, and a key under a
//!   scheme of its own (`skd://`) is resolved by the player's key system;
//!   neither names anything further to fetch.
//! - A playlist — a variant, an I-frame variant, a rendition — is
//!   dropped. The player loads an inline `data:` playlist and parses it
//!   like any other, so the absolute upstream URIs inside it would be
//!   fetched directly, past the proxy, and nothing the proxy writes can
//!   reach inside it.

use url::Url;

use super::m3u8::ProxyOrigin;

/// What the rewrite does with one URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
    /// Already on the proxy, or media that is no network fetch: kept
    /// as written.
    AsWritten,
    /// A network URI: signed and pointed at the proxy.
    Proxied,
    /// A playlist that is no network fetch: its entry is dropped.
    Dropped,
}

/// Where `upstream` goes, for an entry that names a playlist when
/// `playlist` holds and media otherwise.
pub(super) fn reach(upstream: &Url, origin: &ProxyOrigin, playlist: bool) -> Reach {
    if upstream.as_str().starts_with(&origin.base) {
        return Reach::AsWritten;
    }
    match (matches!(upstream.scheme(), "http" | "https"), playlist) {
        (true, _) => Reach::Proxied,
        (false, false) => Reach::AsWritten,
        (false, true) => Reach::Dropped,
    }
}

#[cfg(test)]
#[path = "m3u8_reach_test.rs"]
mod tests;
