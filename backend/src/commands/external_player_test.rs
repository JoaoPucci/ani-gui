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

// — The stream URL is the stream, never an option ————————————————————
//
// The URL goes on the player's command line as a bare argument, last.
// It comes from the provider's answer, so a value that is not an
// http(s) URL — `--script=…`, say, which mpv would run — never
// reaches the player.

#[test]
fn a_stream_url_must_be_http_or_https() {
    assert!(is_stream_url("https://example.com/master.m3u8"));
    assert!(is_stream_url("http://127.0.0.1:9/x/master.m3u8"));
    for bad in [
        "",
        "--script=/tmp/x.lua",
        "-v",
        "file:///etc/passwd",
        "/tmp/x.mp4",
        "C:\\x.mp4",
        "ytdl://x",
        " https://example.com/x",
        "https://",
        "HTTPS://example.com/x",
    ] {
        assert!(!is_stream_url(bad), "{bad:?}");
    }
}

#[cfg(unix)]
#[test]
fn a_stream_url_that_is_not_one_is_refused_and_the_player_never_starts() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("tmp");
    let ran = dir.path().join("ran");
    let player = dir.path().join("player");
    std::fs::write(&player, format!("#!/bin/sh\ntouch '{}'\n", ran.display()))
        .expect("write player");
    std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let mut a = args("--script=/tmp/x.lua");
    a.player_command = player.display().to_string();
    let r = open_external_player(&a);
    assert!(
        matches!(r, Err(AniError::ParseFailed { .. })),
        "expected a refusal, got {r:?}"
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!ran.exists(), "the player must not have been started");
}

proptest::proptest! {
    #[test]
    fn nothing_taken_as_a_stream_url_reads_as_an_option(s in ".*") {
        if is_stream_url(&s) {
            proptest::prop_assert!(!s.starts_with('-'));
            proptest::prop_assert!(s.starts_with("http://") || s.starts_with("https://"));
        }
    }

    #[test]
    fn every_http_url_is_a_stream_url(
        scheme in proptest::sample::select(vec!["http", "https"]),
        host in "[a-z][a-z0-9]{0,10}(\\.[a-z]{2,5})?",
        path in "(/[A-Za-z0-9._~-]{0,8}){0,4}",
    ) {
        let url = format!("{scheme}://{host}{path}");
        proptest::prop_assert!(is_stream_url(&url), "{}", url);
    }
}
