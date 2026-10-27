//! Range downloads — the native path's version of the script's
//! `-e 1-12` loop: one pick, then per-episode resolution and one
//! tool run per episode, in order, stopping at the first failure
//! exactly as the script's own loop dies mid-range.

use crate::app::AppState;
use crate::error::Result;

use super::download::{DownloadArgs, DownloadProgress};
use super::play_native::PickedShow;
use super::play_native_episode::ResolvedEpisode;
use super::play_native_resolve::NativeError;
use super::play_native_walk::pick_native_walk_heard;
use super::providers::{gate_of, Attempt};
use crate::scraper::provider::Provider;

/// `"a-b"` with both halves integers and `a <= b`. Anything else is
/// not a range: the single-episode path keeps its own semantics
/// (integer and fractional tags), and a malformed pair falls through
/// to it as a request that names no row, which dead-ends as the
/// episode's own verdict, [`crate::error::AniError::EpisodeUnavailable`]
/// — the show having been found and only what was asked for missing
/// — instead of silently downloading nothing.
pub(crate) fn episode_range(episode: &str) -> Option<(u32, u32)> {
    let (a, b) = episode.split_once('-')?;
    let a: u32 = a.trim().parse().ok()?;
    let b: u32 = b.trim().parse().ok()?;
    (a <= b).then_some((a, b))
}

/// The range's start as an attempt: the pick and the first episode's
/// stream. The runner moves it between providers, and a provider
/// commits to the range only once it has served that first stream —
/// a primary whose catalogue pages answer but whose stream chain is
/// broken hands the range to the fallback, as a single download
/// would. The remaining episodes resolve against whichever served it.
struct RangeStartAttempt<'r> {
    args: &'r DownloadArgs,
    first: u32,
    quality: &'r str,
    /// The evidence about the entry's last episodes, when the range
    /// knows the entry.
    tail: Option<&'r dyn super::play_native_appended::TailSource>,
    /// The provider whose miss the walk returned — set by the walk,
    /// which knows whose verdict survived.
    answered_by: Option<crate::scraper::provider::ProviderId>,
    /// Where each listing a provider's pick lands is handed, whether or
    /// not its first episode then serves ([`crate::commands::listing_tags`]).
    heard: Option<&'r dyn crate::commands::listing_tags::ListingSink>,
}

#[async_trait::async_trait]
impl Attempt for RangeStartAttempt<'_> {
    /// The pick in the version the range names, the version that served
    /// the first episode, and that episode's stream.
    type Output = (PickedShow, PickedShow, ResolvedEpisode);

    async fn run(
        &mut self,
        provider: &dyn Provider,
    ) -> std::result::Result<(PickedShow, PickedShow, ResolvedEpisode), NativeError> {
        let picked = pick_native_walk_heard(
            provider,
            &self.args.title,
            &self.args.alt_titles,
            self.args.episode_count,
            self.args.year,
            self.args.subtype.as_deref(),
            self.tail,
            self.heard,
        )
        .await?
        .choose(self.args.version);
        // The first episode from the version named, or the next that
        // carries it; the range keeps the version named for the rest,
        // and the version that served names the episode's file.
        let (served, first) = super::play_native_version_modes::resolve_in_any_version(
            provider,
            picked.clone(),
            &self.first.to_string(),
            None,
            &self.args.mode,
            self.quality,
        )
        .await;
        Ok((picked, served, first?))
    }

    fn missed_by(&mut self, provider: crate::scraper::provider::ProviderId) {
        self.answered_by = Some(provider);
    }
}

/// Download episodes `first..=last` of the picked show. The pick and
/// the first episode's stream run against the providers in order;
/// the remaining episodes then resolve against the one that served
/// it, so a range never straddles two catalogues. That provider's
/// breaker hears the start's verdict once and, on a failing later
/// episode, that episode's verdict — the same observed-at stamping
/// as the play path, since every request here rides the same gated
/// transport.
///
/// # Errors
/// The walk's or the failing episode's typed error; the tool's own
/// failures as in [`super::download::spawn_download_tool`].
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
    let at_start =
        super::availability::RowAtStart::read(state, args.kitsu_id.as_deref(), &args.mode).await;
    let remembered = at_start.remembered();
    let tail = super::kitsu_tail::KitsuTail::for_entry(state, args.kitsu_id.as_deref());
    // Every listing the start picks is recorded, whether or not its
    // first episode serves.
    let recorder = crate::commands::listing_tags::Recorder(state);
    let mut attempt = RangeStartAttempt {
        args,
        first,
        quality,
        tail: tail
            .as_ref()
            .map(|t| t as &dyn super::play_native_appended::TailSource),
        answered_by: None,
        heard: Some(&recorder),
    };
    let attempted = match super::providers::run_from(state, remembered, prio, &mut attempt).await {
        Ok(attempted) => attempted,
        Err(ne) => {
            if ne.clean_miss {
                super::availability::stamp_after_native(
                    state,
                    args.kitsu_id.as_deref(),
                    &args.mode,
                    &at_start,
                    super::availability::ResolveVerdict::missed(attempt.answered_by),
                )
                .await;
            }
            return Err(ne.error);
        }
    };
    let (picked, first_served, first_resolved) = attempted.value;
    let (_, played) =
        super::play_native_versions::named_versions(attempted.provider, &first_served);
    // As a play does: a stamp an earlier version wrote with a stitched
    // row's former tag reads right — for every version of the pick,
    // each keying stamps of its own.
    for (hit, episodes) in picked.copies() {
        crate::commands::numbering_offset::retag_from_listing(
            state,
            &crate::scraper::provider::ShowKey::new(attempted.provider, hit.slug.clone())
                .to_string(),
            episodes,
        );
    }
    // The range's first stream is a positive availability fact
    // naming the provider that served it, with the cap the pick's
    // listing paid for — the row the play path writes.
    let extra_tags = super::play_native_numbering::extra_episode_tags(&picked.episodes);
    let (versions, _) = super::play_native_versions::named_versions(attempted.provider, &picked);
    super::availability::stamp_after_native(
        state,
        args.kitsu_id.as_deref(),
        &args.mode,
        &at_start,
        super::availability::ResolveVerdict::served(
            attempted.provider,
            super::play_native_numbering::kitsu_episode_cap(&picked.episodes),
            &extra_tags,
        )
        .with_versions(&versions)
        // The copy that served the first episode carries the mode.
        .with_played(played)
        .with_count_unread(picked.unread_past.is_some()),
    )
    .await;
    let client = attempted.client;
    let gate = gate_of(state, attempted.provider);
    on_progress(DownloadProgress {
        line: format!("Matched {}", picked.hit.title),
    });
    let mut first_resolved = Some((first_served, first_resolved));
    for ep in first..=last {
        // The shape the dock's progress parser consumes — the script's
        // own per-iteration announcement, which drives the
        // "Episode N of M" display.
        on_progress(DownloadProgress {
            line: format!("Playing episode {ep}"),
        });
        let ep_no = ep.to_string();
        let episode_started_at = tokio::time::Instant::now();
        // The first episode was resolved by the attempt that chose
        // the provider; the rest resolve here, against it.
        // Each episode with the version that served it: the version
        // named, or the next that carries the episode in the mode, as
        // the range's start did. The file and the log name that one.
        let (serving, resolved) = match first_resolved.take() {
            Some((served, r)) => (served, Ok(r)),
            None => {
                super::play_native_version_modes::resolve_in_any_version(
                    &*client,
                    picked.clone(),
                    &ep_no,
                    None,
                    &args.mode,
                    quality,
                )
                .await
            }
        };
        let resolved = match resolved {
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
        let file_stem = format!("{} Episode {ep}", serving.hit.title);
        tracing::info!(
            slug = %serving.hit.slug,
            episode = %ep_no,
            dest = %dest.display(),
            "download: spawning tool on natively resolved stream",
        );
        let source = crate::scraper::provider::StreamSource {
            master_url: resolved.master_url,
            referer: resolved.referer,
            subtitles: resolved.subtitles,
        };
        // The sidecars are fetched beside the transfer, while their
        // signed URLs are as fresh as the stream's. The transfer yields
        // to playback the proxy is serving.
        let is_live = || {
            state
                .sessions
                .playback_live(super::download_pacing::PLAYBACK_LIVE_WINDOW)
        };
        // The relay opens the first time a paced run asks for it and
        // closes when the transfer is done with it.
        let download_relay = super::download::DownloadRelay::new(state, &source);
        let relay = || download_relay.url();
        let pacing = super::download_pacing::Pacing::new(
            &is_live,
            super::download_pacing::PACING_POLL,
            &super::download_pacing::PACED_LANE,
        )
        .woken_by(state.sessions.media_noted())
        .charged_to(&state.host_budget)
        .with_relay(&relay);
        super::download_transfer::transfer_with_sidecars(
            &state.proxy_http,
            &state.host_budget,
            &source,
            dest,
            &file_stem,
            Some(quality),
            path_env,
            std::time::Duration::from_secs(60 * 60),
            &mut |line| {
                super::download_tool_output::log_progress_line(line);
                on_progress(DownloadProgress {
                    line: line.to_string(),
                });
            },
            &pacing,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "download_range_test.rs"]
mod tests;
