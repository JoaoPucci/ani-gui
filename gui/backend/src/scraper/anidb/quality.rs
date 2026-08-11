//! Quality selection over a validated master playlist — split from
//! the client for the per-file complexity bar. The policy: one
//! validating fetch on every path, variant selection for a concrete
//! height, soft fallback only on an answered rendition miss.

use crate::error::{AniError, Result};

use super::parse_api::{parse_master_variants, select_variant};
use super::{AnidbClient, AnidbFetch};

pub(super) async fn stream_url<F: AnidbFetch>(
    client: &AnidbClient<F>,
    master_url: &str,
    quality: &str,
) -> Result<String> {
    // One validating fetch on EVERY path, best included: the
    // extracted URL is only a claim until the playlist answers,
    // and an unvalidated claim rides into breaker success,
    // availability, history, and a cached session the proxy
    // cannot load.
    let body = client.content(master_url).await?;
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
        tracing::debug!(
            quality,
            "anidb: quality not served, keeping adaptive master"
        );
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
    match client.content(&rendition).await {
        // An HTML answer on the rendition is an answered miss like
        // a 404 — the validated adaptive master carries the play.
        Ok(body) if is_hls_playlist(&body) => Ok(rendition),
        Ok(_) => {
            tracing::debug!(
                quality,
                "anidb: rendition answered without a playlist, keeping adaptive master"
            );
            Ok(master_url.to_string())
        }
        Err(AniError::Upstream { status })
            if !AniError::Upstream { status }.is_provider_block() =>
        {
            tracing::debug!(
                quality,
                status,
                "anidb: rendition not served, keeping adaptive master"
            );
            Ok(master_url.to_string())
        }
        Err(e) => Err(e),
    }
}

/// The one marker every HLS playlist opens with; anything else is a
/// page, not a stream.
fn is_hls_playlist(body: &str) -> bool {
    body.trim_start().starts_with("#EXTM3U")
}
