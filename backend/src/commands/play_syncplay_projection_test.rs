//! A launch projected onto Syncplay's arguments keeps everything the
//! wrapped player needs.

use super::*;
use crate::commands::external_player::{ExternalPlayerKind, LaunchArgs};

#[test]
fn the_projection_carries_the_tracks_with_the_stream_and_referer() {
    let launch = LaunchArgs {
        stream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: Some("https://embed.example/".into()),
        title: Some("The Show · ep 1".into()),
        player_command: "mpv".into(),
        player_kind: ExternalPlayerKind::Mpv,
        custom_args_template: None,
        subtitle_urls: vec!["https://cdn.example/x/subs/en.vtt".into()],
    };
    let args = syncplay_launch_for(
        launch,
        "syncplay".into(),
        ExternalPlayerKind::Mpv,
        "mpv".into(),
    );
    assert_eq!(args.stream_url, "https://cdn.example/x/master.m3u8");
    assert_eq!(args.referer.as_deref(), Some("https://embed.example/"));
    assert_eq!(
        args.subtitle_urls,
        vec!["https://cdn.example/x/subs/en.vtt".to_string()]
    );
    assert_eq!(args.binary, "syncplay");
    assert_eq!(args.player_binary, "mpv");
}
