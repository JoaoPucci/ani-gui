//! Tests for `crate::external_player`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

fn args(stream: &str) -> LaunchArgs {
    LaunchArgs {
        stream_url: stream.into(),
        referer: None,
        subtitle_url: None,
        title: None,
        player_command: "mpv".into(),
        player_kind: ExternalPlayerKind::Mpv,
        custom_args_template: None,
    }
}

#[test]
fn argv_with_only_stream_is_a_single_arg() {
    let v = build_argv(&args("https://example.com/v.mp4"));
    assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
}

#[test]
fn argv_includes_force_media_title_when_present() {
    let mut a = args("https://example.com/v.mp4");
    a.title = Some("Test Anime Episode 1".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--force-media-title=Test Anime Episode 1".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_emits_sub_file_and_referer_in_the_same_order_as_ani_cli() {
    let mut a = args("https://example.com/master.m3u8");
    a.title = Some("T".into());
    a.subtitle_url = Some("https://example.com/sub.vtt".into());
    a.referer = Some("https://allmanga.to".into());
    let v = build_argv(&a);
    // Order matches play_episode's construction:
    //   --force-media-title=... --sub-file=... --referrer=... <url>
    assert_eq!(
        v,
        vec![
            "--force-media-title=T".to_string(),
            "--sub-file=https://example.com/sub.vtt".to_string(),
            "--referrer=https://allmanga.to".to_string(),
            "https://example.com/master.m3u8".to_string(),
        ]
    );
}

#[test]
fn argv_for_vlc_uses_vlc_flag_syntax() {
    // VLC's flag names differ from mpv: `--meta-title` for the
    // title, `--http-referrer` for the Referer header, and the
    // global `--sub-file` for subtitles. Order matches mpv's:
    // title, sub, referrer, URL last.
    let mut a = args("https://example.com/master.m3u8");
    a.player_kind = ExternalPlayerKind::Vlc;
    a.title = Some("T".into());
    a.subtitle_url = Some("https://example.com/sub.vtt".into());
    a.referer = Some("https://allmanga.to".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--meta-title=T".to_string(),
            "--sub-file=https://example.com/sub.vtt".to_string(),
            "--http-referrer=https://allmanga.to".to_string(),
            "https://example.com/master.m3u8".to_string(),
        ]
    );
}

#[test]
fn argv_for_iina_uses_mpv_prefixed_flags() {
    // IINA wraps mpv on macOS and forwards flags through `--mpv-`,
    // except `--sub-file` which IINA exposes natively.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Iina;
    a.title = Some("T".into());
    a.subtitle_url = Some("https://example.com/sub.vtt".into());
    a.referer = Some("https://allmanga.to".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--mpv-force-media-title=T".to_string(),
            "--sub-file=https://example.com/sub.vtt".to_string(),
            "--mpv-referrer=https://allmanga.to".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_for_custom_kind_substitutes_placeholders() {
    // Custom uses a free-text template the user controls. Tokens
    // are shlex-split, then `{url}`, `{referer}`, `{title}`,
    // `{sub}` are interpolated per token.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Custom;
    a.title = Some("My Show".into());
    a.subtitle_url = Some("https://example.com/sub.vtt".into());
    a.referer = Some("https://allmanga.to".into());
    a.custom_args_template = Some("--ref={referer} --title={title} --sub={sub} {url}".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--ref=https://allmanga.to".to_string(),
            "--title=My Show".to_string(),
            "--sub=https://example.com/sub.vtt".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_for_custom_drops_tokens_with_missing_placeholders() {
    // If the user includes `--sub={sub}` in the template but the
    // current episode has no subtitle, the entire token is
    // dropped — better than emitting `--sub=` with empty value.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Custom;
    a.referer = Some("https://allmanga.to".into());
    // No subtitle, no title.
    a.custom_args_template = Some("--ref={referer} --title={title} --sub={sub} {url}".into());
    let v = build_argv(&a);
    // --title= and --sub= tokens are dropped because their
    // placeholders are missing.
    assert_eq!(
        v,
        vec![
            "--ref=https://allmanga.to".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_for_custom_with_empty_template_falls_back_to_url_only() {
    // A user who picks Custom but leaves the template blank gets
    // a bare URL — not a panic, not an error.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Custom;
    a.custom_args_template = None;
    let v = build_argv(&a);
    assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
}

#[test]
fn launch_args_decode_without_player_kind_field_for_back_compat() {
    // Old client payloads (pre-multi-player) don't include
    // `player_kind`. They must still decode and default to Mpv.
    let json = r#"{
            "stream_url": "https://example.com/v.mp4",
            "referer": null,
            "subtitle_url": null,
            "title": null,
            "player_command": "mpv"
        }"#;
    let a: LaunchArgs = serde_json::from_str(json).expect("decodes with default kind");
    assert_eq!(a.player_kind, ExternalPlayerKind::Mpv);
    assert!(a.custom_args_template.is_none());
}

#[test]
fn open_external_player_with_blank_command_returns_player_spawn_failed() {
    let mut a = args("https://example.com/v.mp4");
    a.player_command = String::new();
    let r = open_external_player(&a);
    match r {
        Err(AniError::PlayerSpawnFailed { binary }) => assert!(binary.is_empty()),
        other => panic!("expected PlayerSpawnFailed, got {other:?}"),
    }
}

#[test]
fn open_external_player_with_unknown_command_carries_binary_name() {
    // The whole point of the new variant: the frontend can name
    // which command failed in the toast.
    let mut a = args("https://example.com/v.mp4");
    a.player_command = "__definitely_not_a_real_player__".into();
    let r = open_external_player(&a);
    match r {
        Err(AniError::PlayerSpawnFailed { binary }) => {
            assert_eq!(binary, "__definitely_not_a_real_player__");
        }
        other => panic!("expected PlayerSpawnFailed, got {other:?}"),
    }
}
