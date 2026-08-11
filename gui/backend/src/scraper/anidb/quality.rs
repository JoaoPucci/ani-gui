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
        Ok(_) => Ok(rendition),
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
