//! Reading the sources endpoint's reply into an embed payload,
//! decrypting its file where the site encrypts it; split from [`super`]
//! so each file stays inside the CRAP gate's per-file bar.

use super::*;

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
