use super::{episode_place, show_folder_name, EpisodePlace, RevealFolder, SHOW_FOLDER_MAX_UNITS};
use proptest::prelude::*;

fn place(dir: &std::path::Path, stem: &str) -> EpisodePlace {
    EpisodePlace {
        dir: dir.to_path_buf(),
        stem: stem.to_string(),
    }
}

#[test]
fn a_show_folder_is_named_as_the_episode_names_its_title() {
    assert_eq!(show_folder_name("Frieren").as_deref(), Some("Frieren"));
    assert_eq!(
        show_folder_name("Re:Zero kara Hajimeru Isekai Seikatsu").as_deref(),
        Some("Re - Zero kara Hajimeru Isekai Seikatsu")
    );
    assert_eq!(show_folder_name("Fate/Zero").as_deref(), Some("Fate-Zero"));
    assert_eq!(
        show_folder_name("Is It Wrong to Pick Up Girls in a Dungeon?").as_deref(),
        Some("Is It Wrong to Pick Up Girls in a Dungeon")
    );
}

#[test]
fn a_show_folder_is_never_hidden_and_never_ends_in_a_dot_or_space() {
    // A leading dot hides a folder on Linux and macOS; Windows drops
    // trailing dots and spaces from the name it creates.
    assert_eq!(
        show_folder_name(".hack//Sign").as_deref(),
        Some("hack--Sign")
    );
    assert_eq!(show_folder_name("Show...").as_deref(), Some("Show"));
    assert_eq!(show_folder_name("  Show . ").as_deref(), Some("Show"));
    assert_eq!(
        show_folder_name("Mr. Osomatsu").as_deref(),
        Some("Mr. Osomatsu")
    );
}

#[test]
fn a_title_that_leaves_no_name_has_no_folder() {
    for title in ["", " ", ".", "..", "...", "???", "\"*\"", " . . "] {
        assert_eq!(show_folder_name(title), None, "{title:?}");
    }
}

#[test]
fn a_device_name_is_not_a_device_as_a_folder() {
    assert_eq!(show_folder_name("CON").as_deref(), Some("CON-"));
    assert_eq!(show_folder_name("nul").as_deref(), Some("nul-"));
    assert_eq!(show_folder_name("Aux.").as_deref(), Some("Aux-"));
    assert_eq!(show_folder_name("COM1").as_deref(), Some("COM1-"));
    assert_eq!(show_folder_name("lpt³").as_deref(), Some("lpt³-"));
    assert_eq!(show_folder_name("CONOUT$").as_deref(), Some("CONOUT$-"));
    assert_eq!(show_folder_name("Nul. Show").as_deref(), Some("Nul- Show"));
    assert_eq!(show_folder_name("con .x").as_deref(), Some("con -x"));
    // A device name that is only the start of a word is a word.
    assert_eq!(show_folder_name("Console").as_deref(), Some("Console"));
    assert_eq!(show_folder_name("COM10").as_deref(), Some("COM10"));
}

#[test]
fn a_long_title_is_cut_to_the_folder_limit_on_a_character() {
    let title = "Isekai de Cheat Skill wo Te ni Shita Ore wa, Genjitsu Sekai wo mo Musou Suru: Level Up wa Jinsei wo Kaeta";
    let name = show_folder_name(title).expect("a name");
    assert!(
        name.encode_utf16().count() <= SHOW_FOLDER_MAX_UNITS,
        "{name}"
    );
    assert!(title.replace(':', " -").starts_with(&name), "{name}");
    // Characters outside the BMP count two units each, and are never
    // split.
    let emoji = "\u{1F338}".repeat(SHOW_FOLDER_MAX_UNITS);
    let name = show_folder_name(&emoji).expect("a name");
    assert_eq!(name.chars().count(), SHOW_FOLDER_MAX_UNITS / 2);
}

#[test]
fn an_episode_goes_in_its_shows_folder() {
    let chosen = tempfile::tempdir().expect("chosen");
    assert_eq!(
        episode_place(chosen.path(), "Re:Zero", "3", None),
        place(&chosen.path().join("Re - Zero"), "Re - Zero Episode 3")
    );
}

#[test]
fn every_copy_of_a_show_shares_its_folder_and_keeps_its_own_name() {
    let chosen = tempfile::tempdir().expect("chosen");
    assert_eq!(
        episode_place(chosen.path(), "JoJo (Uncensored)", "1", None),
        place(&chosen.path().join("JoJo"), "JoJo (Uncensored) Episode 1")
    );
    assert_eq!(
        episode_place(chosen.path(), "JoJo [UNCENSORED]", "2", None),
        place(&chosen.path().join("JoJo"), "JoJo [UNCENSORED] Episode 2")
    );
    assert_eq!(
        episode_place(chosen.path(), "JoJo", "3", None),
        place(&chosen.path().join("JoJo"), "JoJo Episode 3")
    );
}

#[test]
fn an_episode_an_earlier_version_saved_in_the_chosen_folder_is_found_there() {
    let chosen = tempfile::tempdir().expect("chosen");
    std::fs::write(chosen.path().join("Frieren Episode 2.mp4"), b"video").expect("seed");
    assert_eq!(
        episode_place(chosen.path(), "Frieren", "2", None),
        place(chosen.path(), "Frieren Episode 2"),
        "the file from before show folders is the episode"
    );
    assert_eq!(
        episode_place(chosen.path(), "Frieren", "3", None),
        place(&chosen.path().join("Frieren"), "Frieren Episode 3"),
        "an episode not there yet goes in the show's folder"
    );
}

#[test]
fn an_earlier_versions_former_number_in_the_chosen_folder_is_found_there() {
    let chosen = tempfile::tempdir().expect("chosen");
    std::fs::write(chosen.path().join("Show Episode 24.5.mp4"), b"video").expect("seed");
    assert_eq!(
        episode_place(chosen.path(), "Show", "12.5", Some("24.5")),
        place(chosen.path(), "Show Episode 24.5")
    );
}

#[test]
fn the_shows_folder_wins_over_a_copy_in_the_chosen_folder() {
    let chosen = tempfile::tempdir().expect("chosen");
    std::fs::create_dir(chosen.path().join("Show")).expect("folder");
    std::fs::write(chosen.path().join("Show").join("Show Episode 1.mp4"), b"v").expect("seed");
    std::fs::write(chosen.path().join("Show Episode 1.mp4"), b"v").expect("seed");
    assert_eq!(
        episode_place(chosen.path(), "Show", "1", None),
        place(&chosen.path().join("Show"), "Show Episode 1")
    );
}

#[test]
fn an_empty_leftover_or_a_directory_in_the_chosen_folder_is_not_the_episode() {
    let chosen = tempfile::tempdir().expect("chosen");
    std::fs::write(chosen.path().join("Show Episode 1.mp4"), b"").expect("claim");
    std::fs::create_dir(chosen.path().join("Show Episode 2.mp4")).expect("dir");
    for ep in ["1", "2"] {
        assert_eq!(
            episode_place(chosen.path(), "Show", ep, None),
            place(&chosen.path().join("Show"), &format!("Show Episode {ep}"))
        );
    }
}

#[test]
fn a_title_with_no_folder_name_goes_in_the_chosen_folder() {
    let chosen = tempfile::tempdir().expect("chosen");
    assert_eq!(
        episode_place(chosen.path(), "...", "1", None),
        place(chosen.path(), "... Episode 1")
    );
}

#[test]
fn the_reveal_opens_the_shows_folder_once_any_episode_went_there() {
    let chosen = std::path::Path::new("/chosen");
    let show = chosen.join("Show");
    let reveal = RevealFolder::new(chosen);
    assert_eq!(reveal.folder(), chosen, "nothing noted: the chosen folder");
    let mut reveal = RevealFolder::new(chosen);
    reveal.note(chosen);
    assert_eq!(
        reveal.folder(),
        chosen,
        "every episode already in the chosen folder"
    );
    for order in [[chosen, show.as_path()], [show.as_path(), chosen]] {
        let mut reveal = RevealFolder::new(chosen);
        for dir in order {
            reveal.note(dir);
        }
        assert_eq!(reveal.folder(), show, "{order:?}");
    }
}

#[cfg(windows)]
#[test]
fn every_folder_name_is_the_folder_windows_creates() {
    // On NTFS: the folder named is the folder made, spelled the same,
    // directly in the chosen folder, a directory the dock's "open
    // folder" accepts (an existing absolute directory), and an episode
    // written under the path the app uses lands inside it.
    let titles = [
        "Re:Zero kara Hajimeru Isekai Seikatsu",
        "CON",
        "nul.x",
        "COM1",
        "CONIN$",
        "Show.",
        "Show ",
        " .hidden",
        "Fate/Zero",
        "A\\B|C",
        "Is It Wrong?",
        "\"Oshi\" <no> *Ko*",
        "..\\escape",
        "Isekai de Cheat Skill wo Te ni Shita Ore wa, Genjitsu Sekai wo mo Musou Suru: Level Up wa Jinsei wo Kaeta",
    ];
    for title in titles {
        let chosen = tempfile::tempdir().expect("chosen");
        let at = episode_place(chosen.path(), title, "1", None);
        let name = show_folder_name(title).unwrap_or_else(|| panic!("{title:?} has a name"));
        assert_eq!(at.dir, chosen.path().join(&name), "{title:?}");
        std::fs::create_dir_all(&at.dir).unwrap_or_else(|e| panic!("{title:?}: {e}"));
        std::fs::write(at.dir.join(format!("{}.mp4", at.stem)), b"v")
            .unwrap_or_else(|e| panic!("{title:?}: {e}"));
        assert!(at.dir.is_absolute(), "{title:?}");
        assert!(
            std::fs::metadata(&at.dir).is_ok_and(|m| m.is_dir()),
            "{title:?}"
        );
        let list = |dir: &std::path::Path| -> Vec<std::ffi::OsString> {
            std::fs::read_dir(dir)
                .expect("read")
                .map(|e| e.expect("entry").file_name())
                .collect()
        };
        assert_eq!(
            list(chosen.path()),
            vec![std::ffi::OsString::from(&name)],
            "{title:?}: the folder made is spelled as it was named"
        );
        assert_eq!(
            list(&at.dir),
            vec![std::ffi::OsString::from(format!("{}.mp4", at.stem))],
            "{title:?}: the episode is in the folder, under its name"
        );
    }
}

/// What Windows refuses anywhere in a file name.
fn refused_by_windows(c: char) -> bool {
    matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20
}

proptest! {
    /// Whatever the title, a folder name is one path component every
    /// packaged platform creates as spelled: nothing Windows refuses, no
    /// hidden or traversing name, no edge Windows drops, within the
    /// length, not a device.
    #[test]
    fn a_show_folder_name_is_one_portable_component(
        title in "\\PC{0,120}|[\\x00-\\x7f]{0,120}|(CON|PRN|AUX|NUL|COM1|LPT9|con|Nul)[. ]{0,2}[a-z]{0,3}"
    ) {
        let Some(name) = show_folder_name(&title) else {
            return Ok(());
        };
        prop_assert!(!name.chars().any(refused_by_windows), "{name:?}");
        prop_assert!(!name.starts_with('.') && !name.starts_with(char::is_whitespace), "{name:?}");
        prop_assert!(!name.ends_with('.') && !name.ends_with(char::is_whitespace), "{name:?}");
        prop_assert!(name.encode_utf16().count() <= SHOW_FOLDER_MAX_UNITS + 1, "{name:?}");
        let base = name.split('.').next().unwrap_or_default().trim_end().to_uppercase();
        prop_assert!(
            !super::DEVICES.contains(&base.as_str())
                && !(base.len() == 4
                    && (base.starts_with("COM") || base.starts_with("LPT"))
                    && base[3..].chars().all(|c| c.is_ascii_digit())),
            "{name:?}"
        );
        let chosen = std::path::Path::new("/chosen");
        let joined = chosen.join(&name);
        prop_assert_eq!(joined.parent(), Some(chosen), "{:?}", name);
        prop_assert_eq!(show_folder_name(&name), Some(name.clone()), "a name is its own name");
    }

    /// A title already a plain name — letters, digits and inner spaces,
    /// within the length — is its own folder.
    #[test]
    fn a_plain_title_is_its_own_folder(title in "[A-Za-z0-9]{5}( ?[A-Za-z0-9]){0,37}") {
        prop_assert_eq!(show_folder_name(&title), Some(title.clone()));
    }

    /// The folder always holds the episode, whatever the title: the
    /// place is the show's folder directly in the chosen one, or the
    /// chosen folder itself when the title gives no name.
    #[test]
    fn the_place_is_directly_in_the_chosen_folder(title in "\\PC{0,40}", ep in "[0-9]{1,3}") {
        let chosen = std::path::Path::new("/nonexistent-chosen");
        let at = episode_place(chosen, &title, &ep, None);
        match show_folder_name(
            &crate::commands::play_native_version_tag::without_version_tag(&title),
        ) {
            Some(name) => prop_assert_eq!(at.dir, chosen.join(name)),
            None => prop_assert_eq!(at.dir, chosen.to_path_buf()),
        }
    }
}
