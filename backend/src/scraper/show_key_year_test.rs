//! The release year a show key names ([`super::key_year`]).

use super::key_year;
use proptest::prelude::*;

#[test]
fn a_year_among_the_slugs_words_is_the_keys() {
    assert_eq!(
        key_year("hianime:jojos-bizarre-adventure-2000-5182"),
        Some(2000)
    );
    assert_eq!(
        key_year("hianime:the-melancholy-of-haruhi-suzumiya-2009-4640"),
        Some(2009)
    );
    // A bare key from a retired provider names its year the same way.
    assert_eq!(key_year("jojos-bizarre-adventure-2000-77"), Some(2000));
}

#[test]
fn the_numeric_id_after_the_words_is_no_year() {
    assert_eq!(key_year("hianime:black-clover-949"), None);
    assert_eq!(key_year("hianime:show-2000"), None);
    assert_eq!(key_year("hianime:gintama-1237"), None);
}

#[test]
fn a_number_outside_the_years_shows_were_made_in_is_no_year() {
    assert_eq!(key_year("hianime:blade-runner-2049-5"), None);
    assert_eq!(key_year("hianime:mob-psycho-100-926"), None);
    assert_eq!(key_year("hianime:show-1950-3"), None);
}

#[test]
fn a_key_without_words_names_no_year() {
    assert_eq!(key_year("ReooPAxPMsHM4KPMY"), None);
    assert_eq!(key_year(""), None);
    assert_eq!(key_year("hianime:2000"), None);
}

fn words() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[a-z][a-z0-9]{0,6}", 1..4)
}

proptest! {
    /// A year from 1960 to 2039 standing among the words is the key's,
    /// wherever it stands, under any provider label or none.
    #[test]
    fn a_year_word_is_read_wherever_it_stands(
        before in words(), after in prop::collection::vec("[a-z][a-z0-9]{0,6}", 0..3),
        year in 1960u32..2040, id in 1u32..100_000, labelled in any::<bool>(),
    ) {
        let mut parts = before.clone();
        parts.push(year.to_string());
        parts.extend(after.iter().cloned());
        let slug = format!("{}-{id}", parts.join("-"));
        let key = if labelled { format!("hianime:{slug}") } else { slug };
        prop_assert_eq!(key_year(&key), Some(year));
    }

    /// Words that are not a year name none, whatever the numeric id.
    #[test]
    fn words_without_a_year_name_none(w in words(), id in 1u32..100_000) {
        let key = format!("hianime:{}-{id}", w.join("-"));
        prop_assert_eq!(key_year(&key), None);
    }
}
