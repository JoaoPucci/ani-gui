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
        subtitle_urls: Vec::new(),
        subtitles_off: false,
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
    // Only Custom: a blank command for mpv, VLC or IINA runs that
    // type's standard program (see `player_program`), and spawning a
    // real player is not something a test may do.
    let mut a = args("https://example.com/v.mp4");
    a.player_command = String::new();
    a.player_kind = ExternalPlayerKind::Custom;
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

    // Let go once the spawn has found the player busy: a spawn that
    // came to it after the release would exec at once, retry nothing,
    // and prove nothing.
    let queue = crate::held::queue(player.display().to_string());
    let releaser = std::thread::spawn(move || {
        queue.joined_blocking(|| false);
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

#[test]
fn a_blank_command_runs_the_types_standard_program() {
    for blank in ["", "   ", "\t"] {
        assert_eq!(player_program(blank, ExternalPlayerKind::Mpv), Some("mpv"));
        assert_eq!(player_program(blank, ExternalPlayerKind::Vlc), Some("vlc"));
        assert_eq!(
            player_program(blank, ExternalPlayerKind::Iina),
            Some("iina")
        );
    }
}

#[test]
fn a_blank_command_for_custom_runs_nothing() {
    // Custom has no standard program: the user names one.
    assert_eq!(player_program("", ExternalPlayerKind::Custom), None);
    assert_eq!(player_program("  ", ExternalPlayerKind::Custom), None);
}

#[test]
fn a_written_command_runs_as_written_for_every_type() {
    for kind in [
        ExternalPlayerKind::Mpv,
        ExternalPlayerKind::Vlc,
        ExternalPlayerKind::Iina,
        ExternalPlayerKind::Custom,
    ] {
        assert_eq!(player_program("/usr/bin/mpv", kind), Some("/usr/bin/mpv"));
        assert_eq!(player_program("vlc", kind), Some("vlc"));
    }
}

#[test]
fn a_blank_command_fits_its_subtitles_to_the_program_that_runs() {
    // The Windows command-line budget is measured for the program that
    // launches. A blank mpv command launches mpv, so it keeps exactly
    // the tracks the command "mpv" keeps, at every length near the edge.
    for len in 400..600 {
        let url = format!("https://cdn.example/{}", "s".repeat(len));
        let mut blank = args("https://example.com/v.m3u8");
        blank.player_command = String::new();
        blank.subtitle_urls = vec![url; 40];
        let mut named = args("https://example.com/v.m3u8");
        named.subtitle_urls = blank.subtitle_urls.clone();
        assert_eq!(build_argv(&blank), build_argv(&named), "url length {len}");
    }
}

/// A stand-in for the installed-program search: VLC and mpv are
/// installed, IINA is not.
fn installed(name: &str) -> Option<String> {
    match name {
        "vlc" => Some("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe".into()),
        "mpv" => Some("/usr/bin/mpv".into()),
        _ => None,
    }
}

#[test]
fn a_blank_command_launches_the_installed_standard_program() {
    assert_eq!(
        launch_program_with("", ExternalPlayerKind::Vlc, &installed).as_deref(),
        Some("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe")
    );
    assert_eq!(
        launch_program_with("  ", ExternalPlayerKind::Mpv, &installed).as_deref(),
        Some("/usr/bin/mpv")
    );
}

#[test]
fn a_bare_standard_name_launches_that_program_where_it_is_installed() {
    // The name decides, not the type: "vlc" under the mpv type is VLC.
    assert_eq!(
        launch_program_with(" VLC.exe ", ExternalPlayerKind::Mpv, &installed).as_deref(),
        Some("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe")
    );
    assert_eq!(
        launch_program_with("mpv", ExternalPlayerKind::Mpv, &installed).as_deref(),
        Some("/usr/bin/mpv")
    );
}

#[test]
fn a_program_that_is_not_found_is_launched_by_its_name_as_before() {
    assert_eq!(
        launch_program_with("", ExternalPlayerKind::Iina, &installed).as_deref(),
        Some("iina")
    );
    assert_eq!(
        launch_program_with("iina", ExternalPlayerKind::Iina, &installed).as_deref(),
        Some("iina")
    );
}

#[test]
fn a_written_command_is_launched_as_written_and_never_looked_up() {
    let never = |_: &str| -> Option<String> { panic!("a written command is not looked up") };
    for written in [
        "/opt/mpv/mpv",
        "C:\\Tools\\mpv.exe",
        "celluloid",
        "mpv --fs",
    ] {
        assert_eq!(
            launch_program_with(written, ExternalPlayerKind::Mpv, &never).as_deref(),
            Some(written)
        );
    }
    assert_eq!(
        launch_program_with("", ExternalPlayerKind::Custom, &never),
        None
    );
}

#[test]
fn syncplay_keeps_its_own_player_for_a_blank_command() {
    // A blank field passes no --player-path: Syncplay opens the player
    // configured in Syncplay itself, as it always did.
    let never = |_: &str| -> Option<String> { panic!("a blank field is not looked up") };
    for kind in [
        ExternalPlayerKind::Mpv,
        ExternalPlayerKind::Vlc,
        ExternalPlayerKind::Iina,
        ExternalPlayerKind::Custom,
    ] {
        assert_eq!(syncplay_player_with("", kind, &never), "");
        assert_eq!(syncplay_player_with("   ", kind, &never), "");
    }
}

#[test]
fn syncplay_gets_a_bare_name_as_the_installed_program() {
    assert_eq!(
        syncplay_player_with("vlc", ExternalPlayerKind::Vlc, &installed),
        "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe"
    );
    // Not found: the name, as before, for Syncplay's own PATH walk.
    assert_eq!(
        syncplay_player_with("iina", ExternalPlayerKind::Iina, &installed),
        "iina"
    );
    assert_eq!(
        syncplay_player_with("/opt/vlc/vlc", ExternalPlayerKind::Vlc, &installed),
        "/opt/vlc/vlc"
    );
}

#[test]
fn settings_learns_what_a_blank_command_runs_here() {
    assert_eq!(
        standard_program_found(ExternalPlayerKind::Vlc, &installed),
        StandardProgram {
            name: Some("vlc"),
            path: Some("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe".into()),
            supported: true,
        }
    );
    assert_eq!(
        // On macOS, where IINA exists: the next test covers elsewhere.
        standard_program_found_on(
            crate::commands::player_locate::Os::MacOs,
            ExternalPlayerKind::Iina,
            &installed
        ),
        StandardProgram {
            name: Some("iina"),
            path: None,
            supported: true,
        }
    );
    let never = |_: &str| -> Option<String> { panic!("Custom has nothing to look up") };
    assert_eq!(
        standard_program_found(ExternalPlayerKind::Custom, &never),
        StandardProgram {
            name: None,
            path: None,
            supported: true,
        }
    );
}

#[test]
fn iina_is_supported_only_on_macos() {
    use crate::commands::player_locate::Os;
    let never = |_: &str| -> Option<String> { panic!("IINA is not looked up off macOS") };
    for os in [Os::Linux, Os::Windows] {
        assert_eq!(
            standard_program_found_on(os, ExternalPlayerKind::Iina, &never),
            StandardProgram {
                name: Some("iina"),
                path: None,
                supported: false,
            },
            "{os:?}"
        );
    }
    let found = |n: &str| Some(format!("/Applications/{n}"));
    assert_eq!(
        standard_program_found_on(Os::MacOs, ExternalPlayerKind::Iina, &found),
        StandardProgram {
            name: Some("iina"),
            path: Some("/Applications/iina".into()),
            supported: true,
        }
    );
    // mpv and VLC exist everywhere.
    for os in [Os::Linux, Os::Windows, Os::MacOs] {
        assert!(standard_program_found_on(os, ExternalPlayerKind::Vlc, &found).supported);
        assert!(standard_program_found_on(os, ExternalPlayerKind::Mpv, &found).supported);
    }
}
