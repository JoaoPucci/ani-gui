//! Kitsu's own line between the entries it serves a reader who is not
//! signed in and the ones it hides: safe is a rating of G, PG or R, or
//! none at all; anything else is `nsfw`.

use super::*;
use proptest::prelude::*;

/// A ref carrying only an id and a rating — all the rule reads.
fn rated(id: &str, age_rating: Option<&str>) -> KitsuAnimeRef {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "canonical_title": format!("Show {id}"),
        "age_rating": age_rating,
    }))
    .expect("a ref")
}

#[test]
fn the_ratings_kitsu_serves_are_not_adult() {
    for rating in [Some("G"), Some("PG"), Some("R"), None] {
        assert!(!is_nsfw(rating), "{rating:?} is served");
    }
}

#[test]
fn r18_is_adult() {
    // Redo of Healer (Kitsu 42732) answers the mappings lookup with
    // `nsfw: true` and `ageRating: "R18"`, and `/anime/42732` with 404.
    assert!(is_nsfw(Some("R18")));
}

/// A rating as Kitsu could send it: one of its four, or anything else.
fn rating() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("G".to_string())),
        Just(Some("PG".to_string())),
        Just(Some("R".to_string())),
        Just(Some("R18".to_string())),
        "[A-Za-z0-9+-]{0,5}".prop_map(Some),
    ]
}

proptest! {
    #[test]
    fn only_a_rating_off_the_safe_list_is_adult(r in rating()) {
        let safe = matches!(r.as_deref(), None | Some("G" | "PG" | "R"));
        prop_assert_eq!(is_nsfw(r.as_deref()), !safe);
    }

    #[test]
    fn dropping_adult_entries_keeps_every_other_in_order(
        ratings in proptest::collection::vec(rating(), 0..12),
    ) {
        let refs: Vec<KitsuAnimeRef> = ratings
            .iter()
            .enumerate()
            .map(|(i, r)| rated(&i.to_string(), r.as_deref()))
            .collect();
        let kept: Vec<String> = drop_nsfw(refs).into_iter().map(|r| r.id).collect();
        let want: Vec<String> = ratings
            .iter()
            .enumerate()
            .filter(|(_, r)| !is_nsfw(r.as_deref()))
            .map(|(i, _)| i.to_string())
            .collect();
        prop_assert_eq!(kept, want);
    }
}
