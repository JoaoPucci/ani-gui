//! Property coverage for the pure mapping a resolve's cache row is
//! built from.

use super::cached_resolution_for;
use super::MediaKind;
use crate::commands::play_native_resolve::NativeResolved;

proptest::proptest! {
    /// Every field the row reads from the resolve arrives unchanged —
    /// the stream, the show's key and title, the slot — the referer as
    /// the provider named it with the empty string standing for none,
    /// and the kind is always HLS, since the resolve accepted the
    /// stream only because its body opened as a playlist.
    #[test]
    fn the_row_carries_the_resolve_field_for_field(
        slug in "[a-z0-9-]{1,30}",
        title in "[^\\x00]{0,40}",
        master_url in "https://[a-z]{1,10}\\.[a-z]{2,4}/[a-z0-9/._-]{0,30}",
        referer in proptest::option::of("https://[a-z]{1,10}\\.[a-z]{2,4}/"),
        slot in 0u32..10_000,
        cap in proptest::option::of(0u32..10_000),
        offset in 0u32..1000,
        tags in proptest::collection::vec("[a-z0-9.]{1,8}", 0..3),
        tag in proptest::option::of("[a-z0-9.]{1,8}"),
    ) {
        let resolved = NativeResolved {
            slug: slug.clone(),
            title: title.clone(),
            master_url: master_url.clone(),
            episode_cap: cap,
            numbering_offset: offset,
            extra_tags: tags,
            resolved_slot: slot,
            resolved_tag: tag,
            referer: referer.clone(),
            subtitles: Vec::new(),
        };
        let row = cached_resolution_for(&resolved);
        proptest::prop_assert_eq!(row.upstream_url, master_url);
        proptest::prop_assert_eq!(row.referer, referer.unwrap_or_default());
        proptest::prop_assert_eq!(row.show_id, slug);
        proptest::prop_assert_eq!(row.show_title, title);
        proptest::prop_assert_eq!(row.resolved_slot, Some(slot));
        proptest::prop_assert!(matches!(row.media_kind, MediaKind::Hls));
    }
}
