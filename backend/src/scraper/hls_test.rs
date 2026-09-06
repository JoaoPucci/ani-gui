//! The playlist helpers' own tests: variant parsing, the quality
//! arms, and the marker that tells a playlist from a page. The
//! selection flow over a provider is exercised through the anidb
//! client's tests.

use super::*;

/// The master playlist captured from a provider — an HLS master is
/// the same document whichever site served it.
fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/anidb")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn master_variants_parse_sorted_by_height() {
    let variants = parse_master_variants(&fixture("master_op.m3u8"));
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].height, 1080);
    assert_eq!(variants[0].url, "https://cdn.example/op/1080/index.m3u8");
    assert_eq!(variants[1].height, 720);
    assert_eq!(variants[1].url, "https://cdn.example/op/720/index.m3u8");
}

#[test]
fn variant_selection_mirrors_the_scripts_quality_arms() {
    let variants = parse_master_variants(&fixture("master_op.m3u8"));
    assert_eq!(
        select_variant(&variants, "best").map(|v| v.height),
        Some(1080)
    );
    assert_eq!(
        select_variant(&variants, "worst").map(|v| v.height),
        Some(720)
    );
    assert_eq!(
        select_variant(&variants, "720").map(|v| v.height),
        Some(720)
    );
    // A height nobody serves is a miss, not a guess — the caller
    // falls back to the adaptive master.
    assert!(select_variant(&variants, "480").is_none());
    assert!(select_variant(&[], "best").is_none());
}

proptest::proptest! {
    /// The predicate accepts exactly the bodies whose trimmed prefix
    /// is the HLS marker: any leading whitespace is tolerated, and a
    /// body built NOT to open with the marker is refused whatever it
    /// contains further in.
    #[test]
    fn hls_predicate_accepts_exactly_marker_prefixed_bodies(
        ws in "[ \t\r\n]{0,8}",
        rest in "[a-zA-Z0-9 #:=,\n-]{0,64}",
        junk in "[a-zA-Z0-9<][a-zA-Z0-9 #:=,\n-]{0,64}",
    ) {
        let playlist = format!("{ws}#EXTM3U{rest}");
        let page = format!("{ws}{junk}");
        proptest::prop_assert!(is_hls_playlist(&playlist));
        proptest::prop_assert!(!is_hls_playlist(&page));
        proptest::prop_assert!(!is_hls_playlist(&ws));
    }
}
