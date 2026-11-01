//! The row keeps the resolve's sidecar tracks, so a replay offers the
//! same subtitles a fresh resolve did.

use super::*;
use crate::proxy::MediaKind;
use crate::scraper::provider::SubtitleTrack;

#[test]
fn the_row_round_trips_its_sidecar_tracks() {
    let pool = crate::cache::open_in_memory().expect("pool");
    let row = CachedResolution {
        upstream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: "https://embed.example/".into(),
        media_kind: MediaKind::Hls,
        show_id: "the-show-77".into(),
        show_title: "The Show".into(),
        resolved_slot: Some(1),
        subtitles: vec![SubtitleTrack {
            lang: "en".into(),
            label: "English".into(),
            default: true,
            url: "https://cdn.example/x/subs/en.vtt".into(),
        }],
        kitsu_id: None,
        versions: Vec::new(),
        version: None,
        played: None,
        former_episode: None,
        episode_unverified: false,
    };
    put(&pool, "play:test:row", &row);
    assert_eq!(get(&pool, "play:test:row").expect("read"), Some(row));
}

#[test]
fn a_row_written_without_tracks_reads_as_none() {
    // The field is new; a row from the previous schema has none, and
    // the schema bump is what keeps such rows from replaying without
    // the tracks a provider lists.
    let pool = crate::cache::open_in_memory().expect("pool");
    crate::cache::meta_cache_put(
        &pool,
        "play:test:old",
        r#"{"upstream_url":"https://cdn.example/x/master.m3u8","referer":"","media_kind":"hls"}"#,
        3600,
    )
    .expect("put");
    let row = get(&pool, "play:test:old").expect("read").expect("row");
    assert!(row.subtitles.is_empty());
}

/// A row written while the language reader missed the `<code>-<n>.vtt`
/// naming holds each track's label as its language — Steel Ball Run's
/// cached first episode carries `lang: "Portuguese (- Brazil)"` — and
/// a replay would hand those on as they are. Master keys its rows
/// `play:v26:`, so the key must move past it for the tracks to be
/// read again.
#[test]
fn a_row_whose_tracks_took_the_label_as_language_is_not_read() {
    let key = cache_key("t", "sub", "best", "1", None, None, None);
    assert!(
        !key.starts_with("play:v26:"),
        "the key must have moved past play:v26: — {key}"
    );
}
