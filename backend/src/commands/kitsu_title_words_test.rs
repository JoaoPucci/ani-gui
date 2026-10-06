//! Whether a hit's titles share a row's words.

use super::*;
use proptest::prelude::*;
use std::collections::HashMap;

fn hit(canonical: &str, titles: &[(&str, &str)], slug: Option<&str>) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: "1".into(),
        canonical_title: canonical.into(),
        titles: titles
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<HashMap<_, _>>(),
        abbreviated_titles: Vec::new(),
        slug: slug.map(str::to_owned),
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: None,
        average_rating: None,
        subtype: None,
        status: None,
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        cover_image: None,
    }
}

#[test]
fn an_unrelated_first_hit_is_refused() {
    let greenwood = hit(
        "Here is Greenwood",
        &[("en", "Here is Greenwood"), ("en_jp", "Koko wa Green Wood")],
        Some("here-is-greenwood"),
    );
    assert!(!shares_words(
        &["there is also a hole in the student organization"],
        &greenwood
    ));
}

#[test]
fn the_show_a_slug_names_is_accepted_through_any_of_its_titles() {
    let one_piece = hit("One Piece", &[("en", "One Piece")], Some("one-piece"));
    assert!(shares_words(&["one piece part 2"], &one_piece));
    let by_english = hit("Shingeki no Kyojin", &[("en", "Attack on Titan")], None);
    assert!(shares_words(&["attack on titan"], &by_english));
}

#[test]
fn a_hit_or_a_term_with_nothing_to_compare_is_not_refused() {
    let japanese_only = hit("", &[("ja_jp", "ここはグリーン・ウッド")], None);
    assert!(shares_words(&["here is greenwood"], &japanese_only));
    let any = hit("Here is Greenwood", &[], None);
    assert!(shares_words(&["ここ"], &any));
    assert!(shares_words(&["the a an"], &any));
}

proptest! {
    /// A hit carrying the term itself as a title is always accepted.
    #[test]
    fn a_hit_titled_with_the_term_is_accepted(term in "[a-z]{1,8}( [a-z]{1,8}){0,5}") {
        prop_assert!(shares_words(&[term.as_str()], &hit(&term, &[], None)));
    }

    /// Another title on the hit can only add evidence for a hit that
    /// was compared: a hit accepted on words it shared stays accepted
    /// however many titles it gains. (A hit with no comparable title
    /// is accepted on no evidence, and a title it gains can refute it.)
    #[test]
    fn an_extra_title_never_turns_shared_words_into_refusal(
        term in "[a-z]{1,8}( [a-z]{1,8}){0,4}",
        title in "[a-z]{1,8}( [a-z]{1,8}){0,4}",
        extra in "[a-z]{1,8}( [a-z]{1,8}){0,4}",
    ) {
        let hit_alone = hit(&title, &[], None);
        prop_assume!(!words(&title).is_empty() && !words(&term).is_empty());
        let before = shares_words(&[term.as_str()], &hit_alone);
        let after = shares_words(&[term.as_str()], &hit(&title, &[("en", extra.as_str())], None));
        prop_assert!(!before || after);
    }

    /// Case and punctuation are not words: they never change the answer.
    #[test]
    fn case_and_punctuation_do_not_change_the_answer(
        term in "[a-z]{1,8}( [a-z]{1,8}){0,4}",
        title in "[a-z]{1,8}( [a-z]{1,8}){0,4}",
    ) {
        let loud = format!("{}!", title.to_uppercase().replace(' ', ": "));
        prop_assert_eq!(
            shares_words(&[term.as_str()], &hit(&title, &[], None)),
            shares_words(&[term.as_str()], &hit(&loud, &[], None))
        );
    }
}

#[test]
fn a_sequel_marker_is_not_a_shared_word() {
    let other = hit(
        "Seirei Gensouki 2",
        &[("en", "Spirit Chronicles Season 2")],
        Some("seirei-gensouki-2"),
    );
    assert!(!shares_words(&["BLUE LOCK Season 2"], &other));
    let own = hit(
        "Shingeki no Kyojin Season 3",
        &[("en", "Attack on Titan Season 3")],
        None,
    );
    assert!(shares_words(&["Attack on Titan Season 3"], &own));
}

#[derive(serde::Deserialize)]
struct Vector {
    case: String,
    row: Vec<String>,
    hit: VectorHit,
    accept: bool,
}

#[derive(serde::Deserialize)]
struct VectorHit {
    canonical_title: String,
    titles: HashMap<String, String>,
    abbreviated_titles: Vec<String>,
    slug: Option<String>,
}

/// The vectors the frontend's rule runs too: the two sides refuse and
/// accept the same hits for the same rows.
#[test]
fn every_shared_vector_reads_the_same_as_on_the_frontend() {
    let vectors: Vec<Vector> = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/title-words/vectors.json"
    ))
    .expect("vectors");
    assert!(vectors.len() > 80, "the shared vectors are all there");
    let wrong: Vec<&str> = vectors
        .iter()
        .filter(|v| {
            let mut h = hit(&v.hit.canonical_title, &[], v.hit.slug.as_deref());
            h.titles = v.hit.titles.clone();
            h.abbreviated_titles = v.hit.abbreviated_titles.clone();
            let rows: Vec<&str> = v.row.iter().map(String::as_str).collect();
            shares_words(&rows, &h) != v.accept
        })
        .map(|v| v.case.as_str())
        .collect();
    assert!(wrong.is_empty(), "vectors read otherwise: {wrong:?}");
}
