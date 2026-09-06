//! HLS playlists as every provider hands them over: the master's
//! variant rows, the marker that tells a playlist from a page, and
//! the quality selection the resolver runs on the master a provider
//! resolved. Nothing here knows which provider fetched the bytes —
//! the provider's own [`Provider::playlist`] does the fetching, with
//! whatever its CDN requires of the request.
//!
//! The policy: one validating fetch on every path, variant selection
//! for a concrete height, soft fallback only on an answered rendition
//! miss.

use crate::error::{AniError, Result};
use crate::scraper::provider::Provider;

/// One variant row of a master playlist: the stream's vertical
/// resolution and its URI, as the script's `anidb_m3u8` carves them
/// out of `#EXT-X-STREAM-INF` stanzas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterVariant {
    /// Vertical resolution from the stanza's `RESOLUTION=WxH`.
    pub height: u32,
    /// The variant's URI — the stanza's following line, as written.
    pub url: String,
}

/// The master playlist's variant rows, highest resolution first —
/// the order the script's `sort -g -r` produces. Stanzas without a
/// parseable `RESOLUTION` height, and I-frame stanzas (which carry
/// their URI inline and no playable stream), contribute nothing.
pub fn parse_master_variants(m3u8: &str) -> Vec<MasterVariant> {
    let mut out = Vec::new();
    let mut lines = m3u8.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("#EXT-X-STREAM-INF") || line.contains("I-FRAME") {
            continue;
        }
        let Some(height) = line
            .split("RESOLUTION=")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .and_then(|res| res.split('x').nth(1))
            .and_then(|h| h.trim().parse().ok())
        else {
            continue;
        };
        let Some(url) = lines
            .by_ref()
            .find(|l| !l.starts_with('#') && !l.trim().is_empty())
        else {
            break;
        };
        out.push(MasterVariant {
            height,
            url: url.trim().to_string(),
        });
    }
    out.sort_by(|a, b| b.height.cmp(&a.height));
    out
}

/// The variant a quality setting selects, mirroring the script's
/// `select_quality` arms: `best` takes the highest, `worst` the
/// lowest, anything else the variant whose height matches the
/// setting exactly. A miss is `None` — the caller keeps the adaptive
/// master rather than guessing.
pub fn select_variant<'a>(
    variants: &'a [MasterVariant],
    quality: &str,
) -> Option<&'a MasterVariant> {
    match quality {
        "best" => variants.first(),
        "worst" => variants.last(),
        q => variants.iter().find(|v| v.height.to_string() == q),
    }
}

/// The stream URL a quality setting selects from a master playlist,
/// mirroring the script's `select_quality`. `best` keeps the adaptive
/// master URL (hls.js picks levels itself) after one validating
/// fetch; any other setting parses its variants and returns the
/// matching height's URI resolved against the master's URL. Soft only
/// on a SERVED playlist that misses — an unserved height, an
/// unparseable body or variant URI — where the master URL comes back
/// and playback stays adaptive.
///
/// # Errors
/// The fetch's own failure: returning the master URL that just
/// failed would report success upstream — stamping availability,
/// caching a session the player cannot load — and a swallowed 429
/// would record breaker health instead of the rate-limit pause.
pub async fn stream_url<P: Provider + ?Sized>(
    provider: &P,
    master_url: &str,
    quality: &str,
) -> Result<String> {
    // One validating fetch on EVERY path, best included: the
    // extracted URL is only a claim until the playlist answers,
    // and an unvalidated claim rides into breaker success,
    // availability, history, and a cached session the proxy
    // cannot load.
    let body = provider.playlist(master_url).await?;
    if !is_hls_playlist(&body) {
        // 200 with an HTML page passes the status and interstitial
        // checks; success here would ride into the breaker,
        // availability, history, and a cached session for a stream
        // hls.js cannot load.
        return Err(AniError::ParseFailed {
            detail: "master URL did not answer with an HLS playlist".into(),
        });
    }
    if quality == "best" {
        return Ok(master_url.to_string());
    }
    let variants = parse_master_variants(&body);
    let Some(variant) = select_variant(&variants, quality) else {
        tracing::debug!(quality, "quality not served, keeping adaptive master");
        return Ok(master_url.to_string());
    };
    let rendition = match url::Url::parse(master_url).and_then(|base| base.join(&variant.url)) {
        Ok(joined) => joined.to_string(),
        Err(_) => return Ok(master_url.to_string()),
    };
    // The rendition gets its own validating fetch: a dead
    // rendition behind a healthy master must not report success
    // — and an ANSWERED miss must not fail a play the served
    // adaptive master can carry, so that one falls back soft. A
    // refusal, rate limit, or transport failure is not a miss:
    // hls.js would request renditions through the same blocked
    // upstream, and masking it records breaker success and
    // stamps availability, history, and a cached session on a
    // blocked play.
    match provider.playlist(&rendition).await {
        // An HTML answer on the rendition is an answered miss like
        // a 404 — the validated adaptive master carries the play.
        Ok(body) if is_hls_playlist(&body) => Ok(rendition),
        Ok(_) => {
            tracing::debug!(
                quality,
                "rendition answered without a playlist, keeping adaptive master"
            );
            Ok(master_url.to_string())
        }
        Err(AniError::Upstream { status })
            if !AniError::Upstream { status }.is_provider_block() =>
        {
            tracing::debug!(
                quality,
                status,
                "rendition not served, keeping adaptive master"
            );
            Ok(master_url.to_string())
        }
        Err(e) => Err(e),
    }
}

/// The one marker every HLS playlist opens with; anything else is a
/// page, not a stream.
pub fn is_hls_playlist(body: &str) -> bool {
    body.trim_start().starts_with("#EXTM3U")
}

#[cfg(test)]
#[path = "hls_test.rs"]
mod tests;
