use super::portable_name;
use proptest::prelude::*;

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
}
