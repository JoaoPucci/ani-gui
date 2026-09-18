//! The sources answer hianime's megaplay pages lead to: the captions
//! tracks in the clear, and the master playlist as ciphertext under
//! the site's own constants ([`super::megaplay_cipher`]). Which
//! endpoint answers, and for which CDN family, is
//! [`super::megaplay`]'s.

use serde::Deserialize;

use super::embed::EmbedPayload;
use super::megaplay_cipher;
use crate::error::{AniError, Result};
use crate::scraper::provider::{SubtitleTrack, SUBTITLE_URL_CAP};

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
    /// answer of either shape reads; and read by the row rule like
    /// the fields beside it ([`readable_enc`]), so a field of a shape
    /// the reader does not know costs the ciphertext and not the
    /// stream a clear row names.
    #[serde(default, deserialize_with = "readable_enc")]
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

/// The ciphertext the client opens, out of whatever the response put
/// in the field: the string the site writes there, and nothing of any
/// other shape. A number, a list or an object where the ciphertext
/// belongs is read as no ciphertext at all, so the answer is judged
/// by its clear rows — and refused for the stream it lacks when they
/// name none — rather than failed whole for a field it never opens
/// while a clear row names the stream.
fn readable_enc<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<serde_json::Value>::deserialize(deserializer)
        .map(|listed| listed.and_then(|value| value.as_str().map(str::to_owned)))
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

/// The stream out of the ciphertext the site answers with: the
/// plaintext its own constants open the blob to
/// ([`super::megaplay_cipher`]), which is what the clear `sources`
/// field used to carry — one object naming the file, or a list of
/// them read by its first fetchable row ([`file_rows`]).
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
    let plain = megaplay_cipher::opened(enc).map_err(undecodable)?;
    let listed: serde_json::Value = serde_json::from_slice(&plain)
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
