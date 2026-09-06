//! The show key: what a stored id says about whose id it is.

use super::*;

#[test]
fn anidb_keys_are_the_bare_slug_every_existing_row_holds() {
    let key = ShowKey::new(ProviderId::Anidb, "one-piece-69");
    assert_eq!(key.to_string(), "one-piece-69");
    assert_eq!(ShowKey::parse("one-piece-69"), key);
}

#[test]
fn another_providers_key_carries_its_label() {
    let key = ShowKey::new(ProviderId::Hianime, "cowboy-bebop-1281");
    assert_eq!(key.to_string(), "hianime:cowboy-bebop-1281");
    assert_eq!(ShowKey::parse("hianime:cowboy-bebop-1281"), key);
}

#[test]
fn an_unknown_prefix_is_part_of_an_anidb_slug() {
    // Only a label the app knows is a prefix; anything else is the
    // bare id it always was, colon and all.
    let key = ShowKey::parse("weird:thing-12");
    assert_eq!(key.provider, ProviderId::Anidb);
    assert_eq!(key.slug, "weird:thing-12");
}

#[test]
fn the_search_term_is_the_slugs_words_for_either_provider() {
    assert_eq!(
        ShowKey::parse("cowboy-bebop-1281").search_term().as_deref(),
        Some("cowboy bebop")
    );
    assert_eq!(
        ShowKey::parse("hianime:cowboy-bebop-1281")
            .search_term()
            .as_deref(),
        Some("cowboy bebop")
    );
    assert_eq!(
        ShowKey::parse("vDTSJHSpYnrkZnAvG").search_term(),
        None,
        "an allanime-era id is not slug-shaped and carries no words"
    );
}

#[test]
fn a_label_parses_back_to_its_provider_and_anything_else_is_anidb() {
    assert_eq!(ProviderId::from_label("hianime"), ProviderId::Hianime);
    assert_eq!(ProviderId::from_label("anidb.app"), ProviderId::Anidb);
    assert_eq!(
        ProviderId::from_label(""),
        ProviderId::Anidb,
        "an absent label is the default every existing caller means"
    );
    assert_eq!(ProviderId::from_label("something else"), ProviderId::Anidb);
}

proptest::proptest! {
    /// Any provider, any slug: the key round-trips through its string.
    #[test]
    fn a_key_round_trips_through_its_string(
        hianime in proptest::bool::ANY,
        words in proptest::collection::vec("[a-z0-9]{1,8}", 1..6),
        id in 0u64..1_000_000,
    ) {
        let provider = if hianime { ProviderId::Hianime } else { ProviderId::Anidb };
        let key = ShowKey::new(provider, format!("{}-{id}", words.join("-")));
        proptest::prop_assert_eq!(ShowKey::parse(&key.to_string()), key);
    }
}
