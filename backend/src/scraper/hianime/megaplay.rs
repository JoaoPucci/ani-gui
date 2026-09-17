//! The megaplay embed host: its page carries no payload, and its
//! player asks the site for the sources by the media id the page's
//! player element carries, for the CDN family the page's own URL
//! names. The answer names the captions tracks in the clear and the
//! master playlist as ciphertext; the CDN checks the embed host's
//! origin as `Referer` on every fetch of them, like zokoanime's.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut as _, KeyIvInit as _};
use base64::Engine as _;
use serde::Deserialize;

use super::embed::EmbedPayload;
use crate::error::{AniError, Result};
use crate::scraper::provider::{SubtitleTrack, SUBTITLE_URL_CAP};

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

/// The key the site's player opens its sources answer with, as its
/// script carries it: sixteen ASCII bytes, zero-padded to the
/// cipher's width. The site's own constant, used there for its
/// segment tokens as well, so a rotation shows up in both at once.
const SOURCES_KEY: &[u8; 16] = b"i?LMTAx0Q6,:}50U";

/// The initialisation vector beside it, likewise the site's own
/// constant: fixed for every answer, which is why one captured
/// ciphertext is enough to hold the pair to account.
const SOURCES_IV: &[u8; 16] = b"W0;27ToaUpl_P%\'c";

/// The cipher the answer is under.
type SourcesCipher = cbc::Decryptor<aes::Aes256>;

/// How the site writes the ciphertext out: base64 over the URL
/// alphabet, padding optional — the answers seen carry none, and one
/// that carried it would be the same bytes.
const SOURCES_B64: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
    &base64::alphabet::URL_SAFE,
    base64::engine::general_purpose::NO_PAD
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
);

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

/// The CDN family an embed URL names: the [`CDN_FAMILY`] key of its
/// query. `None` for a URL that names none — the site's default
/// family, which its player asks for by leaving the key off.
fn cdn_family(embed: &url::Url) -> Option<String> {
    embed
        .query_pairs()
        .find(|(key, _)| key.as_ref() == CDN_FAMILY)
        .map(|(_, family)| family.into_owned())
}

/// A listed file: an object naming the stream in its `file`.
#[derive(Deserialize)]
struct WireFile {
    #[serde(default)]
    file: String,
}

/// A listed track. `kind` tells captions from the player's other
/// tracks (thumbnails); the language is not listed, only the label.
#[derive(Deserialize)]
struct WireTrack {
    #[serde(default)]
    file: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    default: bool,
}

/// The wire shape of the sources response. Both of its fields are
/// read row by row, so a row in a shape the reader does not know
/// costs that row and nothing else: a lone megaplay server is never
/// skipped over a listed fallback, or its subtitle metadata, having
/// changed shape.
#[derive(Deserialize)]
struct WireResponse {
    #[serde(default, deserialize_with = "readable_sources")]
    sources: Vec<WireFile>,
    /// The stream, as the site writes it now: the `sources` object
    /// encrypted under the site's own constants ([`decrypted_file`]).
    /// Read only when nothing in the clear names a stream, so an
    /// answer of either shape reads.
    #[serde(default)]
    enc: Option<String>,
    #[serde(default, deserialize_with = "readable_tracks")]
    tracks: Vec<WireTrack>,
}

/// The source rows the client reads, out of whatever the response
/// put in the field: the site hands either one object with a `file`
/// or a list of them, the list being where it offers a choice — a
/// rendition first, fallbacks behind it. A row that is not a source
/// in the known shape is dropped, and the rows that read keep the
/// response's order, so one such row costs its own rendition and not
/// the stream the rows beside it name.
///
/// A row has to be an object, which is narrower than the derive
/// alone: serde reads a struct from a list too, taking its fields in
/// declaration order, so `["https://…"]` would otherwise name a
/// stream the site never listed. This field decides what the player
/// is pointed at, so it reads only the shape the site sends.
fn readable_sources<'de, D>(deserializer: D) -> std::result::Result<Vec<WireFile>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let listed = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(listed.map(file_rows).unwrap_or_default())
}

/// The source rows a value carries, by the rule
/// [`readable_sources`] describes. The plaintext of an encrypted
/// answer is that same value, so the two shapes are read by one rule.
fn file_rows(listed: serde_json::Value) -> Vec<WireFile> {
    let rows = match listed {
        serde_json::Value::Array(rows) => rows,
        one => vec![one],
    };
    rows.into_iter()
        .filter(serde_json::Value::is_object)
        .filter_map(|row| serde_json::from_value(row).ok())
        .collect()
}

/// The track rows the client reads, out of whatever the response put
/// in the field. Anything that is not a list yields no tracks; a row
/// that is not a track in the known shape is dropped, and the rows
/// that read keep the response's order.
fn readable_tracks<'de, D>(deserializer: D) -> std::result::Result<Vec<WireTrack>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let listed = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(serde_json::Value::Array(rows)) = listed else {
        return Ok(Vec::new());
    };
    Ok(rows
        .into_iter()
        .filter_map(|row| serde_json::from_value(row).ok())
        .collect())
}

/// The payload out of a sources response. The stream is the first
/// listed source naming an absolute http(s) URL, the rows before it
/// naming nothing fetchable being stepped over ([`readable_sources`]);
/// and, where the clear rows name no such URL, the first the site's
/// ciphertext names ([`decrypted_file`]), which is where the site
/// puts the stream now. A body that is not the response, or one whose
/// sources name no such URL in either place, is the site having
/// changed what it hands the client, never an episode without a
/// stream. Tracks that are not
/// captions, or whose file the transport cannot fetch, are left out,
/// and so are rows the reader cannot read at all
/// ([`readable_tracks`]) and rows whose file runs longer than any a
/// CDN signs ([`SUBTITLE_URL_CAP`]): the hand-offs put every track
/// URL on the player's command line, and one such row would fail
/// the hand-off for an episode whose stream is fine.
///
/// # Errors
/// [`AniError::ParseFailed`] for a body without a fetchable source.
pub fn parse_sources(json: &str) -> Result<EmbedPayload> {
    let undecodable = |what: &str| AniError::ParseFailed {
        detail: format!("megaplay sources: {what}"),
    };
    let wire: WireResponse =
        serde_json::from_str(json).map_err(|_| undecodable("body is not the response"))?;
    let clear = wire
        .sources
        .into_iter()
        .map(|f| f.file)
        .find(|file| is_fetchable(file));
    let src = match (clear, wire.enc.as_deref()) {
        (Some(src), _) => src,
        (None, Some(enc)) => decrypted_file(enc, &undecodable)?,
        (None, None) => return Err(undecodable("no source the transport can fetch")),
    };
    let subtitles = wire
        .tracks
        .into_iter()
        .filter(|t| {
            if !is_captions(&t.kind) || !is_fetchable(&t.file) {
                return false;
            }
            let bounded = t.file.len() <= SUBTITLE_URL_CAP;
            if !bounded {
                tracing::debug!(
                    label = %t.label,
                    len = t.file.len(),
                    "megaplay sources: subtitle file longer than any a CDN signs, row dropped"
                );
            }
            bounded
        })
        .map(|t| SubtitleTrack {
            lang: lang_of_track(&t.file, &t.label),
            label: t.label,
            default: t.default,
            url: t.file,
        })
        .collect();
    Ok(EmbedPayload { src, subtitles })
}

/// The stream out of the ciphertext the site answers with: base64
/// over the URL alphabet, AES-256-CBC under the site's own constants
/// ([`SOURCES_KEY`], [`SOURCES_IV`]), opening to what the clear
/// `sources` field used to carry — one object naming the file, or a
/// list of them read by its first fetchable row ([`file_rows`]).
///
/// Every way this fails is the site having changed something, and
/// each says which: the encoding, the cipher, the constants, the
/// shape behind them, or where the file points. None of them is an
/// episode without a stream, so all of them are parse failures.
///
/// # Errors
/// [`AniError::ParseFailed`], by way of `undecodable`, for a blob
/// that does not open to a file the transport can fetch.
fn decrypted_file(enc: &str, undecodable: &impl Fn(&str) -> AniError) -> Result<String> {
    let mut bytes = SOURCES_B64
        .decode(enc)
        .map_err(|_| undecodable("the encrypted sources are not base64"))?;
    // Checked before the cipher so that a blob of the wrong length
    // and one the constants do not open are told apart: the unpadding
    // refuses both with the one error.
    if bytes.is_empty() || bytes.len() % SOURCES_IV.len() != 0 {
        return Err(undecodable(
            "the encrypted sources are not whole cipher blocks",
        ));
    }
    let mut key = [0u8; 32];
    key[..SOURCES_KEY.len()].copy_from_slice(SOURCES_KEY);
    let plain = SourcesCipher::new(&key.into(), SOURCES_IV.into())
        .decrypt_padded_mut::<Pkcs7>(&mut bytes)
        .map_err(|_| undecodable("the encrypted sources do not open under the site's key"))?;
    let listed: serde_json::Value = serde_json::from_slice(plain)
        .map_err(|_| undecodable("the decrypted sources are not the sources"))?;
    file_rows(listed)
        .into_iter()
        .map(|f| f.file)
        .find(|file| is_fetchable(file))
        .ok_or_else(|| undecodable("the decrypted sources name no source the transport can fetch"))
}

/// The language a track carries: the three-letter code its file name
/// ends in (`track_0_eng.vtt`, `track_2_Latin_American_spa.vtt`) —
/// the site lists no code, only a label — or else the label itself,
/// or `und` when there is neither.
#[must_use]
pub fn lang_of_track(file: &str, label: &str) -> String {
    let name = file
        .rsplit('/')
        .next()
        .unwrap_or(file)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let stem = name
        .strip_suffix(".vtt")
        .or_else(|| name.strip_suffix(".VTT"))
        .unwrap_or(name);
    let code = stem.rsplit('_').next().unwrap_or_default();
    if code.len() == 3 && code.bytes().all(|b| b.is_ascii_lowercase()) {
        return code.to_string();
    }
    let label = label.trim();
    if label.is_empty() {
        "und".to_string()
    } else {
        label.to_string()
    }
}

/// Whether a listed track is a subtitle track, by the kind the
/// player types it with.
fn is_captions(kind: &str) -> bool {
    matches!(
        kind.trim().to_ascii_lowercase().as_str(),
        "captions" | "subtitles"
    )
}

/// Whether the transport can fetch `src`: an absolute URL under
/// http or https, nothing else.
fn is_fetchable(src: &str) -> bool {
    url::Url::parse(src).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}
