//! The transfer and its sidecars, together.
//!
//! A provider's subtitle URLs are signed like its stream and expire
//! on their own, and a transfer can run for an hour. Fetched after
//! it, every track of a long download could be refused, and the
//! episode would land without the subtitles it was resolved with.
//! So the sidecar phase starts with the transfer, while the URLs
//! are as fresh as the stream's, and the two run side by side, each
//! under its own bound: the tool's deadline, and the phase's. Both
//! downloads — the single episode and each episode of a range —
//! come through here, so they share the one ordering.
//!
//! A track that arrives is staged, not published: it waits in its
//! scratch, and takes its name only once the transfer has
//! succeeded. A sidecar beside a failed download would be one the
//! next attempt keeps as the user's own — a file with bytes at the
//! name is never replaced — so a transfer that fails drops every
//! staged track with its scratch, and the names stay free for the
//! retry.

use super::download::SidecarClaim;
use crate::error::Result;
use crate::scraper::provider::StreamSource;
use std::path::{Path, PathBuf};

/// Run the download tool on `source` and fetch its sidecar tracks
/// beside it, returning the sidecar paths written. A transfer that
/// fails ends the phase: the error surfaces without waiting on a
/// track that is stalling, and every track staged so far is dropped
/// with its scratch, the names untaken. A transfer that finishes
/// first waits for the tracks still in flight, under the phase's
/// own deadline, and then the staged tracks are installed at their
/// names.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn transfer_with_sidecars<F>(
    client: &reqwest::Client,
    source: &StreamSource,
    dest: &Path,
    file_stem: &str,
    quality: Option<&str>,
    path_env: &str,
    timeout: std::time::Duration,
    on_line: &mut F,
) -> Result<Vec<PathBuf>>
where
    F: FnMut(&str) + Send,
{
    let mut sidecars = std::pin::pin!(super::download::stage_sidecar_subtitles_with(
        client,
        &source.subtitles,
        source.referer.as_deref(),
        dest,
        file_stem,
        super::download::SIDECAR_PHASE_DEADLINE,
        super::download::SIDECAR_FETCH_CONCURRENCY,
    ));
    let mut transfer = std::pin::pin!(super::download::spawn_download_tool(
        source, dest, file_stem, quality, path_env, timeout, on_line,
    ));
    let mut staged = None;
    let transferred = loop {
        tokio::select! {
            outcome = &mut transfer => break outcome,
            claims = &mut sidecars, if staged.is_none() => staged = Some(claims),
        }
    };
    // A failure returns here, and the staged claims — held in
    // `staged` or still inside the phase — drop with their scratches.
    transferred?;
    let staged = match staged {
        Some(claims) => claims,
        None => sidecars.await,
    };
    Ok(install_staged(staged))
}

/// Install every staged track at its name, in the order given, and
/// return the names filled. A name taken since the claim is the
/// user's and the track is dropped with its scratch; an install that
/// fails otherwise is logged and skipped, since the episode is
/// delivered and that is the transfer.
pub(crate) fn install_staged(staged: Vec<SidecarClaim>) -> Vec<PathBuf> {
    let mut written = Vec::with_capacity(staged.len());
    for claim in staged {
        let path = claim.target().to_path_buf();
        match claim.install() {
            Ok(()) => written.push(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                tracing::info!(path = %path.display(), "download: subtitle already present, kept");
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "download: subtitle write failed");
            }
        }
    }
    written
}

#[cfg(test)]
#[path = "download_transfer_test.rs"]
mod tests;
