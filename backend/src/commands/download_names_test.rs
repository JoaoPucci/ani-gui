use super::{episode_stem, file_safe, portable_name};
use proptest::prelude::*;

/// What Windows refuses anywhere in a file name: the nine reserved
/// characters and the control characters below space.
fn refused_by_windows(c: char) -> bool {
    matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20
}

#[test]
fn a_colon_reads_as_a_dash_between_the_words_it_joined() {
    assert_eq!(
        file_safe("Dragon Ball Super: Beerus"),
        "Dragon Ball Super - Beerus"
    );
    assert_eq!(file_safe("Re:Zero"), "Re - Zero");
    assert_eq!(file_safe("Steins;Gate 0 : Part"), "Steins;Gate 0 - Part");
    assert_eq!(file_safe("Show:"), "Show -");
}

#[test]
fn the_other_reserved_characters_become_a_hyphen_or_go() {
    assert_eq!(file_safe("Fate/Zero"), "Fate-Zero");
    assert_eq!(file_safe("A\\B|C"), "A-B-C");
    assert_eq!(file_safe("Is It Wrong?"), "Is It Wrong");
    assert_eq!(file_safe("\"Oshi\" <no> *Ko*"), "Oshi no Ko");
    assert_eq!(file_safe("Tab\there"), "Tabhere");
}

#[test]
fn an_episode_of_a_title_with_a_colon_is_named_as_one_file() {
    // On Windows a colon in a path names a hidden stream of the file
    // named up to it: "Dragon Ball Super: Beerus Episode 4.mp4" wrote
    // an empty "Dragon Ball Super" and the episode was lost.
    let dest = tempfile::tempdir().expect("dest");
    assert_eq!(
        episode_stem(dest.path(), "Dragon Ball Super: Beerus", "4", None),
        "Dragon Ball Super - Beerus Episode 4"
    );
}

#[test]
fn a_title_that_starts_with_a_device_name_before_a_dot_is_not_that_device() {
    // Windows 10 reads a name whose part before the first dot is a
    // device (CON, NUL, COM1, ...) as that device, extension or not.
    let dest = tempfile::tempdir().expect("dest");
    assert_eq!(
        episode_stem(dest.path(), "Nul. Show", "1", None),
        "Nul- Show Episode 1"
    );
    assert_eq!(
        episode_stem(dest.path(), "com1.Part", "2", None),
        "com1-Part Episode 2"
    );
    assert_eq!(
        episode_stem(dest.path(), "Con .x", "3", None),
        "Con -x Episode 3"
    );
    // A device name that is only the start of a longer word, or a
    // title with no dot at all, is an ordinary name.
    assert_eq!(
        episode_stem(dest.path(), "Mr. Osomatsu", "1", None),
        "Mr. Osomatsu Episode 1"
    );
    assert_eq!(
        episode_stem(dest.path(), "Console. Wars", "1", None),
        "Console. Wars Episode 1"
    );
    assert_eq!(episode_stem(dest.path(), "Aux", "1", None), "Aux Episode 1");
    assert_eq!(
        episode_stem(dest.path(), "COM0.x", "1", None),
        "COM0.x Episode 1"
    );
}

#[test]
fn the_superscript_numbered_ports_are_devices_too() {
    // Windows reserves COM¹-³ and LPT¹-³ alongside the plain-digit ports.
    let dest = tempfile::tempdir().expect("dest");
    assert_eq!(
        episode_stem(dest.path(), "COM¹.x", "1", None),
        "COM¹-x Episode 1"
    );
    assert_eq!(
        episode_stem(dest.path(), "lpt³.y", "1", None),
        "lpt³-y Episode 1"
    );
}

#[cfg(unix)]
#[test]
fn an_earlier_spelling_is_never_looked_for_outside_the_folder() {
    // A provider title with a slash could only have been an earlier
    // download's name if it named a subfolder, which no version
    // created; looking it up joins it as a path and can reach a file
    // outside the folder the download writes to.
    let root = tempfile::tempdir().expect("root");
    let dest = root.path().join("downloads");
    std::fs::create_dir(&dest).expect("dest");
    std::fs::write(root.path().join("Show Episode 1.mp4"), b"x").expect("outside file");
    assert_eq!(
        episode_stem(&dest, "../Show", "1", None),
        "..-Show Episode 1"
    );
}

#[cfg(windows)]
#[test]
fn a_failed_downloads_leftover_does_not_hand_its_broken_name_out_again() {
    // What a colon title left on Windows before: an empty file named up
    // to the colon, with the episode's name as a stream of it. The
    // provider's spelling then looks like a file that exists.
    let dest = tempfile::tempdir().expect("dest");
    std::fs::write(
        dest.path().join("Dragon Ball Super: Beerus Episode 4.mp4"),
        b"",
    )
    .expect("NTFS takes the colon path as a stream");
    assert!(
        dest.path().join("Dragon Ball Super").is_file(),
        "the write left the empty file the stream belongs to"
    );
    assert!(
        std::fs::symlink_metadata(dest.path().join("Dragon Ball Super: Beerus Episode 4.mp4"))
            .is_ok(),
        "the stream answers a lookup under the provider's spelling"
    );
    assert_eq!(
        episode_stem(dest.path(), "Dragon Ball Super: Beerus", "4", None),
        "Dragon Ball Super - Beerus Episode 4"
    );
}

#[cfg(unix)]
#[test]
fn an_episode_an_earlier_version_saved_under_the_raw_title_keeps_that_name() {
    // Unix accepts the colon, so earlier versions' downloads there sit
    // under the title as the provider spelled it: that file is the
    // episode, and the download recognises it instead of writing a
    // second copy beside it.
    let dest = tempfile::tempdir().expect("dest");
    std::fs::write(dest.path().join("Show: Part Episode 1.mp4"), b"x").expect("seed");
    assert_eq!(
        episode_stem(dest.path(), "Show: Part", "1", None),
        "Show: Part Episode 1"
    );
}

#[test]
fn a_title_that_starts_with_a_dot_does_not_name_a_hidden_file() {
    // Linux and macOS hide a file whose name starts with a dot, so an
    // episode of ".hack//Sign" saved as ".hack--Sign Episode 1.mp4"
    // was a download the user could not see.
    let dest = tempfile::tempdir().expect("dest");
    assert_eq!(
        episode_stem(dest.path(), ".hack//Sign", "1", None),
        "hack--Sign Episode 1"
    );
    assert_eq!(
        episode_stem(dest.path(), "..Show", "2", None),
        "Show Episode 2"
    );
    // The dots and the spaces between them go together, so the name
    // does not start with a space either.
    assert_eq!(
        episode_stem(dest.path(), ". . Show", "3", None),
        "Show Episode 3"
    );
    // A title that opens on a dot and on nothing else Windows keeps
    // reads as the same name a title without the dot gives — the rule
    // the show's folder follows.
    assert_eq!(
        episode_stem(dest.path(), ".hack", "1", None),
        episode_stem(dest.path(), "hack", "1", None)
    );
    // Windows' device names are checked on what is left.
    assert_eq!(
        episode_stem(dest.path(), "..CON.x", "1", None),
        "CON-x Episode 1"
    );
}

#[test]
fn a_title_of_only_dots_names_the_episode_alone() {
    let dest = tempfile::tempdir().expect("dest");
    assert_eq!(episode_stem(dest.path(), "...", "1", None), "Episode 1");
    assert_eq!(episode_stem(dest.path(), ". . .", "4", None), "Episode 4");
}

#[test]
fn a_title_whose_name_was_never_hidden_keeps_its_name() {
    // Only a name that starts with a dot is hidden: a dot further in,
    // or after a space, was always a visible file and stays the name
    // earlier versions gave it.
    let dest = tempfile::tempdir().expect("dest");
    assert_eq!(
        episode_stem(dest.path(), "Mr. Osomatsu", "1", None),
        "Mr. Osomatsu Episode 1"
    );
    assert_eq!(
        episode_stem(dest.path(), " .hack", "1", None),
        " .hack Episode 1"
    );
    assert_eq!(
        episode_stem(dest.path(), "Show...", "1", None),
        "Show... Episode 1"
    );
}

#[test]
fn an_episode_an_earlier_version_saved_under_a_dotted_name_keeps_it() {
    // The user's file is the episode: it is recognised under the name
    // it has rather than renamed, or downloaded a second time beside it.
    let dest = tempfile::tempdir().expect("dest");
    std::fs::write(dest.path().join(".hack--Sign Episode 1.mp4"), b"x").expect("seed");
    assert_eq!(
        episode_stem(dest.path(), ".hack//Sign", "1", None),
        ".hack--Sign Episode 1"
    );
    // Numbered the way an earlier version numbered it, too.
    std::fs::write(dest.path().join(".hack--Sign Episode 26.mp4"), b"x").expect("seed");
    assert_eq!(
        episode_stem(dest.path(), ".hack//Sign", "2", Some("26")),
        ".hack--Sign Episode 26"
    );
    // A title of only dots had its dots in the name.
    std::fs::write(dest.path().join("... Episode 3.mp4"), b"x").expect("seed");
    assert_eq!(episode_stem(dest.path(), "...", "3", None), "... Episode 3");
    // An episode with no earlier file gets the visible name.
    assert_eq!(
        episode_stem(dest.path(), ".hack//Sign", "4", None),
        "hack--Sign Episode 4"
    );
}

#[test]
fn the_visible_name_found_on_disk_wins_over_a_dotted_one() {
    // Both spellings on disk: the one this version writes is the one
    // it keeps naming the episode by.
    let dest = tempfile::tempdir().expect("dest");
    std::fs::write(dest.path().join("hack--Sign Episode 7.mp4"), b"x").expect("seed");
    std::fs::write(dest.path().join(".hack--Sign Episode 7.mp4"), b"x").expect("seed");
    assert_eq!(
        episode_stem(dest.path(), ".hack//Sign", "1", Some("7")),
        "hack--Sign Episode 7"
    );
}

#[cfg(unix)]
#[test]
fn an_earlier_raw_spelling_that_starts_with_a_dot_is_still_the_episode() {
    let dest = tempfile::tempdir().expect("dest");
    std::fs::write(dest.path().join(".hack: Roots Episode 1.mp4"), b"x").expect("seed");
    assert_eq!(
        episode_stem(dest.path(), ".hack: Roots", "1", None),
        ".hack: Roots Episode 1"
    );
}

#[test]
fn an_ascii_tag_keeps_its_own_spelling() {
    assert_eq!(portable_name("pt-BR").as_deref(), Some("pt-BR"));
    assert_eq!(portable_name("en").as_deref(), Some("en"));
}

#[test]
fn runs_outside_the_alphabet_become_one_hyphen_and_edges_are_trimmed() {
    assert_eq!(portable_name("zh_Hans").as_deref(), Some("zh-Hans"));
    assert_eq!(portable_name("  en  1 ").as_deref(), Some("en-1"));
    assert_eq!(portable_name("--en--").as_deref(), Some("en"));
    assert_eq!(portable_name("Ïƒ"), None);
    assert_eq!(portable_name(""), None);
}

proptest! {
    /// A portable name is never empty, is spelled in the alphabet,
    /// never starts or ends with a hyphen, never doubles one, and an
    /// ASCII tag already in that shape comes back as it is.
    #[test]
    fn a_portable_name_is_in_the_alphabet_or_none(tag in "\\PC{0,12}") {
        match portable_name(&tag) {
            None => prop_assert!(!tag.chars().any(|c| c.is_ascii_alphanumeric())),
            Some(name) => {
                prop_assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
                prop_assert!(!name.starts_with('-') && !name.ends_with('-'));
                prop_assert!(!name.contains("--"));
                let kept: String = tag.chars().filter(char::is_ascii_alphanumeric).collect();
                let names: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
                prop_assert_eq!(kept, names, "the alphanumerics survive in order");
            }
        }
    }

    #[test]
    fn a_tag_already_portable_is_its_own_name(tag in "[A-Za-z0-9]+(-[A-Za-z0-9]+){0,3}") {
        let name = portable_name(&tag);
        prop_assert_eq!(name.as_deref(), Some(tag.as_str()));
    }

    /// Whatever the title, nothing Windows refuses survives.
    #[test]
    fn a_file_safe_name_holds_nothing_windows_refuses(name in "\\PC{0,24}|[\\x00-\\x7f]{0,24}") {
        let safe = file_safe(&name);
        prop_assert!(!safe.chars().any(refused_by_windows), "{safe:?}");
    }

    /// A title Windows already accepts keeps its own spelling, so a
    /// name the app gave before this rule is the name it gives now.
    #[test]
    fn a_name_windows_accepts_is_its_own_file_safe_name(name in "[^<>:\"/\\\\|?*\\x00-\\x1f]{0,24}") {
        prop_assert_eq!(file_safe(&name), name);
    }

    /// Whatever the title, the name a new download takes is never a
    /// hidden file.
    #[test]
    fn a_new_episodes_name_never_starts_with_a_dot(
        title in "[. ]{0,4}(\\PC{0,16}|[\\x00-\\x7f]{0,16})",
        episode in "[0-9]{1,4}",
    ) {
        let dest = tempfile::tempdir().expect("dest");
        let stem = episode_stem(dest.path(), &title, &episode, None);
        prop_assert!(!stem.starts_with('.'), "{title:?} -> {stem:?}");
        prop_assert!(stem.ends_with(&format!("Episode {episode}")), "{stem:?}");
    }

    /// A title whose file-safe name does not start with a dot names its
    /// episode as it always has, so no visible download changes name.
    #[test]
    fn a_name_that_was_never_hidden_is_unchanged(
        title in "[^.<>:\"/\\\\|?*\\x00-\\x1f][^<>:\"/\\\\|?*\\x00-\\x1f]{0,16}",
    ) {
        let dest = tempfile::tempdir().expect("dest");
        prop_assert_eq!(
            episode_stem(dest.path(), &title, "1", None),
            super::not_a_device(format!("{title} Episode 1"))
        );
    }

    #[test]
    fn making_a_name_file_safe_twice_changes_nothing_more(name in "\\PC{0,24}|[\\x00-\\x7f]{0,24}") {
        let once = file_safe(&name);
        prop_assert_eq!(file_safe(&once), once);
    }
}
