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
fn an_ascii_tag_keeps_its_own_spelling() {
    assert_eq!(portable_name("pt-BR").as_deref(), Some("pt-BR"));
    assert_eq!(portable_name("en").as_deref(), Some("en"));
}

#[test]
fn runs_outside_the_alphabet_become_one_hyphen_and_edges_are_trimmed() {
    assert_eq!(portable_name("zh_Hans").as_deref(), Some("zh-Hans"));
    assert_eq!(portable_name("  en  1 ").as_deref(), Some("en-1"));
    assert_eq!(portable_name("--en--").as_deref(), Some("en"));
    assert_eq!(portable_name("σ"), None);
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
    fn a_file_safe_name_holds_nothing_windows_refuses(name in "\\PC{0,24}|[ -~]{0,24}") {
        let safe = file_safe(&name);
        prop_assert!(!safe.chars().any(refused_by_windows), "{safe:?}");
    }

    /// A title Windows already accepts keeps its own spelling, so a
    /// name the app gave before this rule is the name it gives now.
    #[test]
    fn a_name_windows_accepts_is_its_own_file_safe_name(name in "[^<>:\"/\\\\|?*\\x00-\\x1f]{0,24}") {
        prop_assert_eq!(file_safe(&name), name);
    }

    #[test]
    fn making_a_name_file_safe_twice_changes_nothing_more(name in "[ -~]{0,24}") {
        let once = file_safe(&name);
        prop_assert_eq!(file_safe(&once), once);
    }
}
