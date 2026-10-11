//! The scratch file a transfer writes to beside its target, and the
//! guard that removes it unless publication consumed it.

/// Owns the scratch file for as long as the transfer might still be
/// using it, and removes it otherwise.
///
/// Cancellation is why this is a type rather than a cleanup call.
/// Aborting the download drops the future wherever it happens to be
/// awaiting, so no branch below that point runs — and the file left
/// behind is not a marker but most of an episode. A drop runs anyway.
///
/// Publication is the one exit that stands the guard down. A
/// successful one ends with the scratch consumed, and a tool that
/// handed over a finished file has already cleaned its own derived
/// names — so the drop's sweep would read every entry in the
/// destination to find nothing, once per episode of a range download,
/// on whatever filesystem the user chose. The flag is set after
/// publication answers, never before it: set ahead of the call, it
/// disarms the guard for the failures too, which is the off-by-one-
/// branch this struct once shipped. And it is set on the scratch
/// being gone rather than on the answer, because publication ignores
/// a failed removal on its success arms — a delivered episode is not
/// an error — and disarming on the report alone would leave that one
/// failure's episode-sized file hidden in the folder for good.
pub(super) struct Scratch {
    pub(super) path: std::path::PathBuf,
    /// Names a retry retired. Their removal was attempted at
    /// retirement, and that attempt can fail without saying so — a
    /// file still held open without delete sharing on Windows — so
    /// the guard owns them until the end. A prefix forgotten is a
    /// partial hidden in the destination for good, because nothing
    /// else will ever know to remove it.
    pub(super) retired: Vec<std::path::PathBuf>,
    pub(super) published: std::sync::atomic::AtomicBool,
}

impl Scratch {
    pub(super) fn new(dest: &std::path::Path) -> Self {
        Self {
            path: scratch_path(dest),
            retired: Vec::new(),
            published: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Point at a fresh file, dropping whatever the last one was —
    /// the retry path, where yt-dlp's output is condemned and ffmpeg
    /// needs a name nothing can confuse with it. The condemned name
    /// is retired, not forgotten: the discard here is an attempt.
    pub(super) fn renew(&mut self, dest: &std::path::Path) {
        self.discard();
        let old = std::mem::replace(&mut self.path, scratch_path(dest));
        self.retired.push(old);
    }

    /// Remove the scratch file and everything a tool derived from its
    /// name.
    ///
    /// The `-o` path is not where a transfer's bytes are. yt-dlp's
    /// `--part` is on by default, so they go to `<scratch>.part`, with
    /// `<scratch>.ytdl` holding fragment-resume state and a
    /// `<scratch>.part-FragN` per fragment in flight beside it. A kill
    /// lands among those, and removing only the `-o` path removes the
    /// one name nothing was written to.
    ///
    /// Enumerating that list here would be the same mistake as
    /// reconstructing which spellings are one file: correct until the
    /// tool adds a suffix, and unfixable without knowing it had. The
    /// scratch name carries a uuid this run generated, so a prefix
    /// sweep of the destination is exact — nothing else can hold that
    /// prefix, and nothing derived from the path can lose it.
    pub(super) fn discard(&self) {
        self.sweep(true);
    }

    /// One pass over the destination, removing every entry that
    /// carries a prefix this guard owns: the retired names always,
    /// and the current one unless publication already consumed it.
    /// Costs nothing when there is nothing owned — the common
    /// published case, where `retired` is empty and the read is
    /// skipped entirely.
    pub(super) fn sweep(&self, include_current: bool) {
        let mut prefixes: Vec<&std::ffi::OsStr> = Vec::new();
        if include_current {
            let _ = std::fs::remove_file(&self.path);
            if let Some(name) = self.path.file_name() {
                prefixes.push(name);
            }
        }
        for old in &self.retired {
            let _ = std::fs::remove_file(old);
            if let Some(name) = old.file_name() {
                prefixes.push(name);
            }
        }
        if prefixes.is_empty() {
            return;
        }
        let Some(dir) = self.path.parent() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if prefixes
                .iter()
                .any(|p| name.as_encoded_bytes().starts_with(p.as_encoded_bytes()))
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.published.load(std::sync::atomic::Ordering::Relaxed) {
            // The episode is delivered and the current scratch is
            // provably gone — but a retry may have retired a name
            // whose removal failed, and delivery does not license
            // orphaning it. Free when no retry happened, which is
            // nearly every download.
            self.sweep(false);
            return;
        }
        self.discard();
    }
}

/// Where a transfer writes before it is anyone's episode.
///
/// Beside the target rather than in a temp directory, because
/// publishing is a rename or a link and both need the same
/// filesystem. Carries `.mp4` because yt-dlp and ffmpeg both choose a
/// container from the extension, and a scratch name they cannot read
/// as MP4 changes what they produce.
pub(super) fn scratch_path(dest: &std::path::Path) -> std::path::PathBuf {
    dest.join(format!(
        ".ani-gui-{}-{}.part.mp4",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ))
}
