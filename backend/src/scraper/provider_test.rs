//! The show key: what a stored id says about whose id it is.

use super::*;

#[test]
fn a_bare_id_names_no_provider() {
    // Every row anidb.app's resolves wrote holds its bare slug, and
    // the allanime-era rows before them their opaque ids. The app
    // walks neither provider; the key says so, and keeps the id.
    for id in ["one-piece-69", "vDTSJHSpYnrkZnAvG", ""] {
        let key = ShowKey::parse(id);
        assert_eq!(key.provider, None, "{id:?}");
        assert_eq!(key.slug, id);
        assert_eq!(key.to_string(), id, "a bare id prints as it was stored");
    }
}

#[test]
fn every_known_providers_key_carries_its_label() {
    for &provider in ProviderId::ALL {
        let key = ShowKey::new(provider, "cowboy-bebop-1281");
        assert_eq!(
            key.to_string(),
            format!("{}:cowboy-bebop-1281", provider.label())
        );
        assert_eq!(ShowKey::parse(&key.to_string()), key);
    }
}

#[test]
fn another_providers_key_carries_its_label() {
    let key = ShowKey::new(ProviderId::Hianime, "cowboy-bebop-1281");
    assert_eq!(key.to_string(), "hianime:cowboy-bebop-1281");
    assert_eq!(ShowKey::parse("hianime:cowboy-bebop-1281"), key);
}

#[test]
fn an_unknown_prefix_is_part_of_a_bare_id() {
    // Only a label the app knows is a prefix; anything else is the
    // bare id it always was, colon and all.
    let key = ShowKey::parse("weird:thing-12");
    assert_eq!(key.provider, None);
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
fn a_label_parses_back_to_its_provider_and_anything_else_to_none() {
    for &provider in ProviderId::ALL {
        assert_eq!(ProviderId::from_label(provider.label()), Some(provider));
    }
    assert_eq!(
        ProviderId::from_label(""),
        None,
        "an absent label names nobody"
    );
    assert_eq!(ProviderId::from_label("something else"), None);
}

proptest::proptest! {
    /// Any provider, any slug: the key round-trips through its string.
    #[test]
    fn a_key_round_trips_through_its_string(
        hianime in proptest::bool::ANY,
        words in proptest::collection::vec("[a-z0-9]{1,8}", 1..6),
        id in 0u64..1_000_000,
    ) {
        let slug = format!("{}-{id}", words.join("-"));
        let key = if hianime {
            ShowKey::new(ProviderId::Hianime, slug)
        } else {
            ShowKey::parse(&slug)
        };
        proptest::prop_assert_eq!(ShowKey::parse(&key.to_string()), key);
    }
}

#[test]
fn a_provider_id_serializes_as_the_renderers_identifier() {
    // The identifier the frontend's `StreamProvider` names — `anidb`,
    // not the label `anidb.app` a show key carries — so a response
    // field typed on one side matches the other without a map.
    assert_eq!(
        serde_json::to_string(&ProviderId::Anidb).expect("json"),
        r#""anidb""#
    );
    assert_eq!(
        serde_json::from_str::<ProviderId>(r#""hianime""#).expect("json"),
        ProviderId::Hianime
    );
}
