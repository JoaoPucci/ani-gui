//! The megaplay embed host: its page carries no payload, and its
//! player asks the site for the sources by the media id the page's
//! player element carries, for the CDN family the page's own URL
//! names. What that endpoint answers is read in
//! [`super::megaplay_sources`]; the CDN checks the embed host's
//! origin as `Referer` on every fetch of the stream, like
//! zokoanime's.

/// The element whose `data-id` is the media id.
const PLAYER_ELEMENT: &str = "id=\"megaplay-player\"";

/// The endpoint the player asks for the sources, under the page's
/// own origin.
const SOURCES_PATH: &str = "/stream/getSourcesNew";

/// The query key naming the CDN family a page was served for. The
/// site lists one megaplay server per family — `tcdn`, `bcdn`, and
/// the default a URL names by carrying no `s` at all — and the
/// page's player appends its page's own family to every sources
/// request, which is what makes the answer name that family's hosts.
const CDN_FAMILY: &str = "s";

/// The CDN families whose streams the client cannot serve, by the
/// name the site selects them with ([`CDN_FAMILY`]). `tcdn`'s
/// renditions list their segments as real image files, the transport
/// stream hidden behind a fixed prefix the site's own player strips
/// before it feeds them to the decoder. The proxy hands a segment to
/// the player as it fetched it, so a stream of that family plays
/// nothing — and everything ahead of the segment works, so nothing
/// earlier in the chain says so.
const UNSERVED_FAMILIES: &[&str] = &["tcdn"];

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
/// asked for the media `id` and for the CDN family the embed URL
/// names ([`CDN_FAMILY`]), as the page's own player asks it. The
/// rest of the page's query is the page's business and is left
/// behind. `None` for an embed URL that is not an absolute http(s)
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
    if let Some(family) = cdn_family(&parsed) {
        asked.query_pairs_mut().append_pair(CDN_FAMILY, &family);
    }
    Some(asked.into())
}

/// Whether the client can serve what the family an embed URL names
/// streams ([`UNSERVED_FAMILIES`]). True for a URL that names none,
/// and for one that is not a URL at all: the question is what the
/// family the page was served for streams, and a URL naming no
/// family names the site's default, whose segments are bytes the
/// proxy passes through.
///
/// The key is megaplay's; no other embed host the client reads
/// selects anything with it.
#[must_use]
pub fn served_cdn(embed_url: &str) -> bool {
    url::Url::parse(embed_url)
        .ok()
        .and_then(|embed| cdn_family(&embed))
        .is_none_or(|family| !UNSERVED_FAMILIES.contains(&family.as_str()))
}

/// The CDN family an embed URL names: the [`CDN_FAMILY`] key of its
/// query. `None` for a URL that names none — the site's default
/// family, which its player asks for by leaving the key off.
fn cdn_family(embed: &url::Url) -> Option<String> {
    embed
        .query_pairs()
        .find(|(key, _)| key.as_ref() == CDN_FAMILY)
        .map(|(_, family)| family.into_owned())
}
