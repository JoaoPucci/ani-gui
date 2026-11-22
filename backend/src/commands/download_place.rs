//! Where an episode's download goes inside the folder the user chose:
//! a folder of its own per show, named for the show, with the episode
//! and its sidecars in it — the layout Plex, Jellyfin and Kodi read a
//! series library in.
//!
//! The folder takes the title the file's name is made from, through
//! the same [`file_safe`](super::download_names::file_safe), so the
//! folder and the names inside it read alike — without a version tag:
//! a provider lists a season's uncensored copy as "… (Uncensored)",
//! a range can take one episode from each copy, and both are the same
//! show, so they share its folder while each file keeps the name of
//! the copy it came from. A folder name is a whole
//! path component rather than the start of a longer name, which a file
//! name never is, so it answers four more questions the stem never had
//! to — see [`show_folder_name`].
//!
//! Earlier versions saved every episode straight into the chosen
//! folder. Those files stay where they are: the user's files are not
//! the app's to move. An episode one of them already holds is found
//! there, and the download ends with the same "already here" a file in
//! the show's folder gives, its sidecars going beside it.

use std::path::{Path, PathBuf};

/// The longest a show's folder name is, in UTF-16 code units — the unit
/// Windows measures names and paths in.
///
/// The folder adds its name to every path under it, and the episode's
/// own name already carries the same title. A packaged Windows build
/// runs yt-dlp and ffmpeg, which are not guaranteed to reach past the
/// 260-unit `MAX_PATH` where the system has not opted into long paths,
/// so a long light-novel title, counted twice, would carry an episode
/// past it. Eighty keeps the folder readable and leaves the file's
/// name the room it had before folders existed.
pub(crate) const SHOW_FOLDER_MAX_UNITS: usize = 80;

/// The names Windows opens as a device whatever follows them: a folder
/// called `CON` cannot be created at all, and `CON.x` is the console.
const DEVICES: [&str; 6] = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];

/// The folder a show's downloads go in, or `None` when its title leaves
/// no name to give one.
///
/// It is the title made [`file_safe`](super::download_names::file_safe),
/// then:
///
/// - **One space between words.** A run of whitespace reads as a single
///   space, so a title spelled with spacing aside is the one folder —
///   the version tag of "… (Uncensored) Season 2" leaves its own space
///   beside the ones either side of it, and the copy is the same show.
/// - **No leading dot.** A folder whose name starts with one is hidden
///   on Linux and macOS, and every episode would be hidden with it
///   (`.hack//Sign`).
/// - **No trailing dot or space**, and no leading space. Windows drops
///   them from a name when it creates the folder, so the folder made
///   and the path the app goes on to use would be two spellings of
///   it — the second one wrong whenever the path is long enough for
///   the system to pass it through untouched.
/// - **At most [`SHOW_FOLDER_MAX_UNITS`]**, cut on a character, then
///   trimmed again.
/// - **Not a device name.** The part before the first dot, ignoring
///   case and trailing spaces, is not one of Windows' devices: a
///   device's name gets a hyphen in place of that dot, or after it
///   when there is no dot.
///
/// What is left being empty, `.` or `..` — a title of dots, or of
/// characters Windows refuses — gives `None`, and the episode goes in
/// the chosen folder itself, as it did before show folders, rather
/// than under a name invented for it.
#[must_use]
pub(crate) fn show_folder_name(title: &str) -> Option<String> {
    let safe = super::download_names::file_safe(title);
    let spaced = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trim_edges(&spaced);
    let mut name = String::with_capacity(trimmed.len());
    let mut units = 0;
    for c in trimmed.chars() {
        units += c.len_utf16();
        if units > SHOW_FOLDER_MAX_UNITS {
            break;
        }
        name.push(c);
    }
    let name = trim_edges(&name);
    if name.is_empty() {
        return None;
    }
    Some(not_a_device(name))
}

/// `s` without leading dots and whitespace, and without trailing dots
/// and whitespace.
fn trim_edges(s: &str) -> &str {
    s.trim_start_matches(|c: char| c == '.' || c.is_whitespace())
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
}

/// `name` made not to be a device, as [`show_folder_name`] says.
fn not_a_device(name: &str) -> String {
    let (base, rest) = match name.find('.') {
        Some(dot) => (&name[..dot], Some(&name[dot + 1..])),
        None => (name, None),
    };
    if !is_device(base.trim_end()) {
        return name.to_string();
    }
    match rest {
        Some(rest) => format!("{base}-{rest}"),
        None => format!("{name}-"),
    }
}

fn is_device(base: &str) -> bool {
    let upper = base.to_uppercase();
    let numbered = |prefix: &str| {
        upper.strip_prefix(prefix).is_some_and(|n| {
            let mut digits = n.chars();
            matches!(
                (digits.next(), digits.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    };
    DEVICES.contains(&upper.as_str()) || numbered("COM") || numbered("LPT")
}

/// Where one episode's download goes: the folder, which exists once
/// the caller has created it, and the name the episode takes in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EpisodePlace {
    pub(crate) dir: PathBuf,
    pub(crate) stem: String,
}

/// Where the episode named `episode` of `title` goes in `chosen`: the
/// show's folder — [`show_folder_name`] of `title` without its version
/// tag — under the name
/// [`episode_stem`](super::download_names::episode_stem) gives it
/// there — unless the show's folder does not hold it and `chosen`
/// itself already does, saved there by a version from before show
/// folders, under any name `episode_stem` recognises. Then that file is
/// the episode, and the place is beside it.
///
/// Only a file with something in it counts as the episode there. An
/// empty one is what an interrupted download leaves at its name, and a
/// directory or anything else is not an episode; both are left alone,
/// and the download goes to the show's folder.
#[must_use]
pub(crate) fn episode_place(
    chosen: &Path,
    title: &str,
    episode: &str,
    former: Option<&str>,
) -> EpisodePlace {
    let flat = || EpisodePlace {
        dir: chosen.to_path_buf(),
        stem: super::download_names::episode_stem(chosen, title, episode, former),
    };
    let show = super::play_native_version_tag::without_version_tag(title);
    let Some(folder) = show_folder_name(&show) else {
        return flat();
    };
    let dir = chosen.join(folder);
    let stem = super::download_names::episode_stem(&dir, title, episode, former);
    if !holds_episode(&dir, &stem) {
        let earlier = flat();
        if holds_episode(&earlier.dir, &earlier.stem) {
            return earlier;
        }
    }
    EpisodePlace { dir, stem }
}

/// The folder a download's "open folder" opens, noted episode by
/// episode: the show's folder once any episode went there, and the
/// chosen folder while every one so far was already in it from a
/// version before show folders.
pub(crate) struct RevealFolder {
    chosen: PathBuf,
    show: Option<PathBuf>,
}

impl RevealFolder {
    pub(crate) fn new(chosen: &Path) -> Self {
        Self {
            chosen: chosen.to_path_buf(),
            show: None,
        }
    }

    /// Note that an episode went in `dir`.
    pub(crate) fn note(&mut self, dir: &Path) {
        if dir != self.chosen {
            self.show = Some(dir.to_path_buf());
        }
    }

    /// The folder to open.
    pub(crate) fn folder(self) -> PathBuf {
        self.show.unwrap_or(self.chosen)
    }
}

/// Whether `dir` holds an episode under `stem`: a regular file, not
/// followed through a link, with bytes in it.
fn holds_episode(dir: &Path, stem: &str) -> bool {
    std::fs::symlink_metadata(dir.join(format!("{stem}.mp4")))
        .is_ok_and(|m| m.is_file() && m.len() > 0)
}

#[cfg(test)]
#[path = "download_place_test.rs"]
mod tests;
