//! What a yt-dlp run resumes from: the partial files an earlier run of
//! the same transfer left beside its output.

/// What a yt-dlp run writing to `output` resumes from: the partial
/// files yt-dlp keeps beside it — `<output>.part` for a single format,
/// or one per format, `<stem>.f<id>.<ext>.part`, for a stream it fetches
/// video and audio apart — whose sizes it reports as downloaded when it
/// starts each; nothing when there are none. A fragment that was in
/// flight keeps a partial of its own beside the one it belongs to
/// (`<partial>-Frag<n>.part`), which yt-dlp resumes and reports as
/// downloaded too, and which counts toward that partial's.
pub(super) fn resumed_partials(
    output: &std::path::Path,
) -> crate::commands::download_progress::Resumed {
    let mut resumed = crate::commands::download_progress::Resumed::default();
    let (Some(dir), Some(stem), Some(name)) =
        (output.parent(), output.file_stem(), output.file_name())
    else {
        return resumed;
    };
    let whole = format!("{}.part", name.to_string_lossy());
    let prefix = format!("{}.f", stem.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return resumed;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(len) = entry.metadata().ok().map(|m| m.len()) else {
            continue;
        };
        // A fragment in flight keeps its own partial beside the one it
        // belongs to, `<partial>-Frag<n>.part`, and counts with it.
        let partial = match name.find(".part-Frag") {
            Some(at) if name.ends_with(".part") => &name[..at + ".part".len()],
            Some(_) => continue,
            None => &name[..],
        };
        if partial == whole {
            resumed.whole += len;
        } else if let Some(format) = partial
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(".part"))
            // The id runs up to the extension: it may carry dots itself.
            .and_then(|rest| rest.rsplit_once('.'))
            .map(|(format, _)| format.to_owned())
        {
            *resumed.formats.entry(format).or_default() += len;
        }
    }
    resumed
}
