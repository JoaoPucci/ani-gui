//! The resolve's referer reaches the cache row (and through it the
//! session and the cache-hit replay), instead of the empty string
//! the play command used to write for every provider.

use super::cached_resolution_for;
use crate::commands::play_native_resolve::NativeResolved;

fn native(referer: Option<&str>) -> NativeResolved {
    NativeResolved {
        slug: "the-show-77".into(),
        title: "The Show".into(),
        master_url: "https://cdn.example/x/master.m3u8".into(),
        episode_cap: Some(3),
        numbering_offset: 0,
        extra_tags: Vec::new(),
        resolved_slot: 1,
        resolved_tag: None,
        provider: crate::scraper::provider::ProviderId::Anidb,
        referer: referer.map(str::to_string),
        subtitles: vec![crate::scraper::provider::SubtitleTrack {
            lang: "en".into(),
            label: "English".into(),
            default: true,
            url: "https://cdn.example/x/subs/en.vtt".into(),
        }],
    }
}

#[test]
fn the_cached_row_carries_the_resolves_referer() {
    let row = cached_resolution_for(&native(Some("https://embed.example/")));
    assert_eq!(row.referer, "https://embed.example/");
    assert_eq!(row.upstream_url, "https://cdn.example/x/master.m3u8");
    assert_eq!(row.show_id, "the-show-77");
    assert_eq!(
        row.subtitles
            .iter()
            .map(|t| t.url.as_str())
            .collect::<Vec<_>>(),
        ["https://cdn.example/x/subs/en.vtt"],
        "a replay from the row must offer the same tracks a fresh resolve did"
    );
    assert_eq!(
        cached_resolution_for(&native(None)).referer,
        "",
        "no referer is the empty string the proxy and the HEAD check read as none"
    );
}
