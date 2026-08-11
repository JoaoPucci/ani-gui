//! What yt-dlp leaves behind, and how to tell it went wrong.
//!
//! Both of these read yt-dlp specifically — one matches its own
//! warning line, the other sniffs the MPEG-TS sync byte in a file it
//! named — so they sit beside the downloader rather than with the
//! generic spawn plumbing in [`crate::spawn`].

use std::path::{Path, PathBuf};

/// Remove the files this download announced that are not, in fact,
/// MP4s.
///
/// Two independent bounds, so a good file cannot be eaten by either
/// alone: the path must be one yt-dlp named as its own output for
/// this run *and* sit directly in the directory we handed it, and its
/// own first byte must be the MPEG-TS sync byte `0x47`. A real MP4
/// opens with a length-prefixed `ftyp` box and fails the second test
/// even when it is ours; a concurrent download's file was never
/// announced and fails the first, whatever state its bytes are in.
///
/// No announcement means no deletion. That is the right way to fail
/// here — a mislabeled file left behind is recoverable, someone
/// else's deleted mid-transfer is not.
pub(crate) fn discard_mislabeled(dir: &Path, announced: &[PathBuf]) {
    for path in announced {
        if path.parent() != Some(dir) {
            continue;
        }
        let mut buf = [0u8; 1];
        let is_ts = std::fs::File::open(path)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut buf))
            .is_ok_and(|()| buf[0] == 0x47);
        if is_ts {
            // Best effort: a file we cannot remove is not worth
            // failing the already-failing download over.
            let _ = std::fs::remove_file(path);
        }
    }
}

/// yt-dlp's own report that it left MPEG-TS inside an `.mp4`.
///
/// Anchored to the warning line, not to the stream. yt-dlp emits it
/// through `report_warning`, which prefixes `WARNING:` — measured
/// against 2025.08.11 the line reads:
///
/// ```text
/// WARNING: out: Possible MPEG-TS in MP4 container or malformed
/// AAC timestamps. Install ffmpeg to fix this automatically
/// ```
///
/// Everywhere else in the output the same words would be someone's
/// show title, echoed back by the destination and progress lines. A
/// title is whatever the provider called the entry, so searching the
/// whole stream lets a name decide whether downloads succeed.
///
/// Within the line the match stays on the stable middle: the leading
/// id and the trailing advice are respectively arbitrary and
/// reworded across releases, and pinning either would turn a future
/// yt-dlp into a silent regression.
pub(crate) fn yt_dlp_could_not_repackage(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("WARNING:") && line.contains("Possible MPEG-TS in MP4 container")
    })
}
