//! Range downloads — the native path's version of the script's
//! `-e 1-12` loop: one pick, then per-episode resolution and one
//! tool run per episode, in order, stopping at the first failure
//! exactly as the script's own loop dies mid-range.

use crate::app::AppState;
use crate::error::Result;

use super::download::{spawn_download_tool, DownloadArgs, DownloadProgress};
use super::play_native::PickedShow;
use super::play_native_episode::resolve_episode;
use super::play_native_resolve::NativeError;
use super::play_native_walk::pick_native_walk;
use super::providers::{gate_of, Attempt};
use crate::scraper::provider::Provider;

/// `"a-b"` with both halves integers and `a <= b`. Anything else is
/// not a range: the single-episode path keeps its own semantics
/// (integer and fractional tags), and a malformed pair falls through
/// to the episode resolver's typed `NoResults` instead of silently
/// downloading nothing.
pub(crate) fn episode_range(episode: &str) -> Option<(u32, u32)> {
    let (a, b) = episode.split_once('-')?;
    let a: u32 = a.trim().parse().ok()?;
    let b: u32 = b.trim().parse().ok()?;
    (a <= b).then_some((a, b))
}

/// The pick as an attempt: the runner moves it between providers,
/// and the episodes are then resolved against whichever answered.
struct PickAttempt<'r> {
    args: &'r DownloadArgs,
}

#[async_trait::async_trait]
impl Attempt for PickAttempt<'_> {
    type Output = PickedShow;

    async fn run(
        &mut self,
        provider: &dyn Provider,
    ) -> std::result::Result<PickedShow, NativeError> {
        pick_native_walk(
            provider,
            &self.args.title,
            &self.args.alt_titles,
            self.args.episode_count,
            self.args.year,
            self.args.subtype.as_deref(),
        )
        .await
    }
}

/// Download episodes `first..=last` of the picked show. The pick
/// runs against the providers in order; the episodes then resolve
/// against the one that answered, so a range never straddles two
/// catalogues. That provider's breaker hears the walk's verdict once
/// and, on a failing episode, that episode's verdict — the same
/// observed-at stamping as the play path, since every request here
/// rides the same gated transport.
///
/// # Errors
/// The walk's or the failing episode's typed error; the tool's own
/// failures as in [`spawn_download_tool`].
#[allow(clippy::too_many_arguments)]
pub(crate) async fn download_range<F>(
    state: &AppState,
    args: &DownloadArgs,
    first: u32,
    last: u32,
    quality: &str,
    dest: &std::path::Path,
    path_env: &str,
    on_progress: &mut F,
) -> Result<()>
where
    F: FnMut(DownloadProgress) + Send,
{
    let prio = crate::scraper::gate::ScrapePriority::Interactive;
    // Bounded like the play path's resolve: the walk probes aliases
    // and candidate listings in sequence, each request against its
    // own transport timeout, so an unbounded pick can delay the
    // first transfer past the gate's half-open trial window.
    let remembered = args
        .kitsu_id
        .as_deref()
        .and_then(|id| super::availability::cached_provider(state, id, &args.mode));
    let mut attempt = PickAttempt { args };
    let attempted = super::providers::run_from(state, remembered, prio, &mut attempt)
        .await
        .map_err(|ne| ne.error)?;
    let picked = attempted.value;
    let client = attempted.client;
    let gate = gate_of(state, attempted.provider);
    on_progress(DownloadProgress {
        line: format!("Matched {}", picked.hit.title),
    });
    for ep in first..=last {
        // The shape the dock's progress parser consumes — the script's
        // own per-iteration announcement, which drives the
        // "Episode N of M" display.
        on_progress(DownloadProgress {
            line: format!("Playing episode {ep}"),
        });
        let ep_no = ep.to_string();
        let episode_started_at = tokio::time::Instant::now();
        let resolved = match resolve_episode(&*client, &picked, &ep_no, &args.mode, quality).await {
            Ok(r) => r,
            Err(ne) => {
                let failed: std::result::Result<(), _> = Err(ne);
                if let Some(outcome) = super::play_native_outcome::breaker_outcome(prio, &failed) {
                    let observed_at = failed
                        .as_ref()
                        .err()
                        .and_then(|ne| ne.failed_at)
                        .or_else(|| client.last_attempt_at())
                        .unwrap_or(episode_started_at);
                    gate.record(outcome, observed_at);
                }
                return Err(match failed {
                    Err(ne) => ne.error,
                    Ok(()) => unreachable!(),
                });
            }
        };
        let file_stem = format!("{} Episode {ep}", picked.hit.title);
        tracing::info!(
            slug = %picked.hit.slug,
            episode = %ep_no,
            dest = %dest.display(),
            "download: spawning tool on natively resolved stream",
        );
        let source = crate::scraper::provider::StreamSource {
            master_url: resolved.master_url,
            referer: resolved.referer,
            subtitles: resolved.subtitles,
        };
        spawn_download_tool(
            &source,
            dest,
            &file_stem,
            Some(quality),
            path_env,
            std::time::Duration::from_secs(60 * 60),
            &mut |line| {
                tracing::info!(line = %line, "download.tool.stderr");
                on_progress(DownloadProgress {
                    line: line.to_string(),
                });
            },
        )
        .await?;
        super::download::write_sidecar_subtitles(
            &state.proxy_http,
            &source.subtitles,
            source.referer.as_deref(),
            dest,
            &file_stem,
        )
        .await;
    }
    Ok(())
}

#[cfg(test)]
#[path = "download_range_test.rs"]
mod tests;
