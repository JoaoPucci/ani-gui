//! Tests for `crate::external_player`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

fn args(stream: &str) -> LaunchArgs {
    LaunchArgs {
        stream_url: stream.into(),
        referer: None,
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
fn argv_emits_title_then_referer_then_url() {
    let mut a = args("https://example.com/master.m3u8");
    a.title = Some("T".into());
    a.referer = Some("https://cdn.example".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--force-media-title=T".to_string(),
            "--referrer=https://cdn.example".to_string(),
            "https://example.com/master.m3u8".to_string(),
        ]
    );
}

#[test]
fn argv_for_vlc_uses_vlc_flag_syntax() {
    // VLC's flag names differ from mpv: `--meta-title` for the
    // title, `--http-referrer` for the Referer header. Order
    // matches mpv's: title, referrer, URL last.
    let mut a = args("https://example.com/master.m3u8");
    a.player_kind = ExternalPlayerKind::Vlc;
    a.title = Some("T".into());
    a.referer = Some("https://cdn.example".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--meta-title=T".to_string(),
            "--http-referrer=https://cdn.example".to_string(),
            "https://example.com/master.m3u8".to_string(),
        ]
    );
}

#[test]
fn argv_for_iina_uses_mpv_prefixed_flags() {
    // IINA wraps mpv on macOS and forwards flags through `--mpv-`.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Iina;
    a.title = Some("T".into());
    a.referer = Some("https://cdn.example".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--mpv-force-media-title=T".to_string(),
            "--mpv-referrer=https://cdn.example".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_for_custom_kind_substitutes_placeholders() {
    // Custom uses a free-text template the user controls. Tokens
    // are shlex-split, then `{url}`, `{referer}`, `{title}` are
    // interpolated per token.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Custom;
    a.title = Some("My Show".into());
    a.referer = Some("https://cdn.example".into());
    a.custom_args_template = Some("--ref={referer} --title={title} {url}".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--ref=https://cdn.example".to_string(),
            "--title=My Show".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_for_custom_drops_tokens_with_missing_placeholders() {
    // If the user includes `--title={title}` in the template but
    // the current stream has no title, the entire token is
    // dropped — better than emitting `--title=` with empty value.
    let mut a = args("https://example.com/v.mp4");
    a.player_kind = ExternalPlayerKind::Custom;
    a.referer = Some("https://cdn.example".into());
    // No title.
    a.custom_args_template = Some("--ref={referer} --title={title} {url}".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--ref=https://cdn.example".to_string(),
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

/// Linux only: ETXTBSY on exec of a write-open file is a Linux
/// guarantee. macOS permits the exec, so there is no race to wait
/// out and the assertion below would describe nothing.
#[cfg(target_os = "linux")]
#[test]
fn a_busy_executable_is_waited_out_rather_than_failed() {
    // exec of a file another process still holds open for WRITING
    // fails with ETXTBSY. It is transient by construction — the
    // writer closes microseconds later — so the player and Syncplay
    // spawns, which both go through this helper, retry instead of
    // telling the user their player is broken.
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("tmp");
    let player = dir.path().join("player");
    std::fs::write(&player, "#!/bin/sh\nexit 0\n").expect("write player");
    std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let mut held = std::fs::OpenOptions::new()
        .write(true)
        .open(&player)
        .expect("hold the player open for writing");

    // Self-evidencing: prove the race is live on this host before
    // asserting it gets waited out. Without this a kernel that never
    // reports ETXTBSY would pass the test having exercised nothing.
    let plain = std::process::Command::new(&player).spawn();
    assert!(
        matches!(
            plain.as_ref().err().map(std::io::Error::kind),
            Some(std::io::ErrorKind::ExecutableFileBusy)
        ),
        "a write-open executable must be busy for this test to mean anything: {plain:?}"
    );

    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(15));
        let _ = held.flush();
        drop(held);
    });
    let got = spawn_detached(&mut std::process::Command::new(&player));
    releaser.join().expect("releaser");
    assert!(
        got.is_ok(),
        "a briefly busy executable must be waited out, not reported as a spawn failure: {got:?}"
    );
    // The spawn is detached, so the child execs after this returns —
    // give it that moment before the tempdir (and the script it is
    // about to open) goes away.
    std::thread::sleep(std::time::Duration::from_millis(50));
}
