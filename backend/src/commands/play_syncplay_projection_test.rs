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
        subtitles_off: false,
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

fn launch() -> LaunchArgs {
    LaunchArgs {
        stream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: None,
        title: None,
        player_command: String::new(),
        player_kind: ExternalPlayerKind::Mpv,
        custom_args_template: None,
        subtitle_urls: Vec::new(),
        subtitles_off: false,
    }
}

#[test]
fn a_blank_player_command_wraps_the_types_standard_program() {
    // Syncplay gets the program the "Open in …" item would run, so a
    // blank command is the type's standard one there too.
    for (kind, program) in [
        (ExternalPlayerKind::Mpv, "mpv"),
        (ExternalPlayerKind::Vlc, "vlc"),
        (ExternalPlayerKind::Iina, "iina"),
    ] {
        let args = syncplay_launch_for(launch(), "syncplay".into(), kind, "  ".into());
        assert_eq!(args.player_binary, program, "{kind:?}");
    }
}

#[test]
fn a_blank_custom_command_leaves_the_player_to_syncplay() {
    // No standard program: the flag is skipped and Syncplay's own
    // setting decides, as before.
    let args = syncplay_launch_for(
        launch(),
        "syncplay".into(),
        ExternalPlayerKind::Custom,
        String::new(),
    );
    assert_eq!(args.player_binary, "");
}

#[test]
fn a_written_player_command_is_wrapped_as_written() {
    let args = syncplay_launch_for(
        launch(),
        "syncplay".into(),
        ExternalPlayerKind::Vlc,
        "/opt/vlc/bin/vlc".into(),
    );
    assert_eq!(args.player_binary, "/opt/vlc/bin/vlc");
}
