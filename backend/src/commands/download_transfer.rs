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

use crate::error::Result;
use crate::scraper::provider::StreamSource;
use std::path::{Path, PathBuf};

/// Run the download tool on `source` and fetch its sidecar tracks
/// beside it, returning the sidecar paths written. A transfer that
/// fails ends the phase: the error surfaces without waiting on a
/// track that is stalling, and a sidecar claim dropped unfinished
/// removes its file. A transfer that finishes first waits for the
/// tracks still in flight, under the phase's own deadline.
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
    let mut sidecars = std::pin::pin!(super::download::write_sidecar_subtitles(
        client,
        &source.subtitles,
        source.referer.as_deref(),
        dest,
        file_stem,
    ));
    let mut transfer = std::pin::pin!(super::download::spawn_download_tool(
        source, dest, file_stem, quality, path_env, timeout, on_line,
    ));
    let mut written = None;
    let transferred = loop {
        tokio::select! {
            outcome = &mut transfer => break outcome,
            paths = &mut sidecars, if written.is_none() => written = Some(paths),
        }
    };
    transferred?;
    Ok(match written {
        Some(paths) => paths,
        None => sidecars.await,
    })
}

#[cfg(test)]
#[path = "download_transfer_test.rs"]
mod tests;
