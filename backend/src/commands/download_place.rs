//! Where an episode's download goes inside the folder the user chose:
//! a folder of its own per show, named for the show, with the episode
//! and its sidecars in it — the folder-per-show convention media
//! servers such as Plex, Jellyfin and Kodi organise a series library
//! by.
//!
//! The folder takes the title the file's name is made from, through
//! the same [`file_safe`](super::download_names::file_safe), so the
//! folder and the names inside it read alike — without a version tag:
//! a provider lists a season's uncensored copy as "… (Uncensored)",
//! a range can take one episode from each copy, and both are the same
//! show, so they share its folder while each file keeps the name of
//! the copy it came from. A folder name is a whole path component
//! rather than the start of a longer name, so its edges, its spacing
//! and its length matter in ways the stem's never did, and a device
//! name has to be ruled out on the name alone — see
//! [`show_folder_name`](super::download_folder_name::show_folder_name).
//!
//! Earlier versions saved every episode straight into the chosen
//! folder. Those files stay where they are: the user's files are not
//! the app's to move. An episode one of them already holds is found
//! there, and the download ends with the same "already here" a file in
//! the show's folder gives, its sidecars going beside it.

use std::path::{Path, PathBuf};

pub(crate) use super::download_folder_name::show_folder_name;
#[cfg(test)]
pub(crate) use super::download_folder_name::SHOW_FOLDER_MAX_UNITS;

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
///
/// A show's folder that is a link to a folder elsewhere is followed,
/// as any folder the user made would be: linking a show onto another
/// drive is a choice about where its episodes live. The episode's own
/// name is still never written through a link (`at_target`).
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
