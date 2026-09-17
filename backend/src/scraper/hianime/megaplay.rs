//! The megaplay embed host: its page carries no payload, and its
//! player asks the site for the sources by the media id the page's
//! player element carries, for the content delivery network the
//! request names. What that endpoint answers is read in
//! [`super::megaplay_sources`]; the CDN checks the embed host's
//! origin as `Referer` on every fetch of the stream, like
//! zokoanime's.

/// The element whose `data-id` is the media id.
const PLAYER_ELEMENT: &str = "id=\"megaplay-player\"";

/// The endpoint the player asks for the sources, under the page's
/// own origin.
const SOURCES_PATH: &str = "/stream/getSourcesNew";

/// The query key naming the content delivery network a sources
/// request is answered for. The endpoint honours it for any media
/// id: the id names the episode, this names where it streams from.
const CDN_FAMILY: &str = "s";

/// The one network the client can play a stream from, and so the one
/// every request asks for, whatever network the listing's embed URL
/// named.
///
/// The site streams an episode from three and hands the listing a
/// server per network, its own player asking for the network of the
/// page it runs in. This client is not that player:
///
/// - the default, which a URL names by carrying no `s` at all,
///   answers a master on a host that refuses a playlist fetch which
///   did not come from the site's player;
/// - `tcdn` answers a master whose renditions list their segments as
///   real image files, the transport stream behind a fixed prefix the
///   site's player strips before it feeds them to the decoder, which
///   the proxy does not do — everything ahead of the segment works,
///   so nothing earlier in the chain says so;
/// - `bcdn` answers playlists that fetch and segments that are a
///   transport stream from the first byte, which the proxy passes
///   through unchanged.
///
/// Reading the other two is written down as work of its own in
/// `docs/deferred-work.md`.
const SERVED_FAMILY: &str = "bcdn";

/// The media id off an embed page: the `data-id` of the page's
/// player element, whatever the attribute order. `None` when the
/// page carries no such element or the element no id — a page the
/// reader does not read.
#[must_use]
pub fn media_id(html: &str) -> Option<u64> {
    let at = html.find(PLAYER_ELEMENT)?;
    let start = html[..at].rfind('<')?;
    let end = at + html[at..].find('>')?;
    let tag = &html[start..end];
    let (_, rest) = tag.split_once("data-id=\"")?;
    rest.split('"').next()?.trim().parse().ok()
}

/// The sources endpoint for `embed_url`'s origin — the site serves
/// its player from mirrors, and each answers for its own pages —
/// asked for the media `id` and for the one network the client plays
/// ([`SERVED_FAMILY`]). The embed URL's own query is the site
/// player's business and is left behind, the network it names
/// included. `None` for an embed URL that is not an absolute http(s)
/// URL.
#[must_use]
pub fn sources_url(embed_url: &str, id: u64) -> Option<String> {
    let parsed = url::Url::parse(embed_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let origin = parsed.origin().ascii_serialization();
    let mut asked = url::Url::parse(&format!("{origin}{SOURCES_PATH}")).ok()?;
    asked.query_pairs_mut().append_pair("id", &id.to_string());
    asked
        .query_pairs_mut()
        .append_pair(CDN_FAMILY, SERVED_FAMILY);
    Some(asked.into())
}
