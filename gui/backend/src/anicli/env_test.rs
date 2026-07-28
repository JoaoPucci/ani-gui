//! Tests for `crate::env`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

/// The 4.15 name set, produced through the production probe so every
/// preflight test also exercises the capability gate's positive path.
fn both() -> &'static [&'static str] {
    download_tool_names(
        "case \"$player_function\" in\ndownload) dep_ch_failover \"yt-dlp,ffmpeg\" ;;\nesac",
    )
}

fn split(s: &OsStr) -> Vec<PathBuf> {
    std::env::split_paths(s).collect()
}

fn join(parts: &[&str]) -> OsString {
    let pbs: Vec<PathBuf> = parts.iter().map(PathBuf::from).collect();
    std::env::join_paths(&pbs).expect("join_paths in test fixture")
}

#[test]
fn bundled_bin_is_prepended_to_inherited_path() {
    let bundled = PathBuf::from("/bundle/bin");
    let inherited = join(&["/usr/bin", "/bin"]);
    let got = compose_anicli_path(Some(&bundled), None, Some(&inherited));
    let parts = split(&got);
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
    assert_eq!(parts[1], PathBuf::from("/usr/bin"));
    assert_eq!(parts[2], PathBuf::from("/bin"));
}

#[test]
fn no_bundled_bin_returns_inherited_unchanged() {
    let inherited = join(&["/usr/bin", "/bin"]);
    let got = compose_anicli_path(None, None, Some(&inherited));
    assert_eq!(split(&got), split(&inherited));
}

#[test]
fn path_override_takes_precedence_over_inherited() {
    let inherited = join(&["/usr/bin", "/bin"]);
    let got = compose_anicli_path(None, Some("/shim:/other"), Some(&inherited));
    let parts = split(&got);
    // Override wins; the inherited /usr/bin path is dropped entirely.
    // We don't assert exact equality with the override string because
    // join_paths re-canonicalises the separator per host platform —
    // instead split the override the same way and compare lists.
    let expected: Vec<PathBuf> = std::env::split_paths(OsStr::new("/shim:/other")).collect();
    assert_eq!(parts, expected);
}

#[test]
fn bundled_prepends_path_override_too() {
    let bundled = PathBuf::from("/bundle/bin");
    let got = compose_anicli_path(Some(&bundled), Some("/shim"), None);
    let parts = split(&got);
    assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
    assert_eq!(parts[1], PathBuf::from("/shim"));
}

#[test]
fn no_bundled_no_inherited_falls_back_to_default() {
    let got = compose_anicli_path(None, None, None);
    let parts = split(&got);
    let expected: Vec<PathBuf> = std::env::split_paths(OsStr::new(FALLBACK_PATH)).collect();
    assert_eq!(parts, expected);
}

#[test]
fn ensure_download_tool_returns_ok_when_executable_in_first_dir() {
    let path = std::env::join_paths(["/bundle/bin", "/usr/bin"].map(PathBuf::from)).unwrap();
    let r = ensure_download_tool_in_path(both(), &path, |p| {
        p == Path::new("/bundle/bin/ffmpeg") || p == Path::new("/bundle/bin/ffmpeg.exe")
    });
    assert!(r.is_ok(), "got: {r:?}");
}

#[test]
fn ensure_download_tool_returns_ok_when_executable_in_later_dir() {
    let path = std::env::join_paths(["/no/ffmpeg/here", "/usr/bin"].map(PathBuf::from)).unwrap();
    let r = ensure_download_tool_in_path(both(), &path, |p| {
        p == Path::new("/usr/bin/ffmpeg") || p == Path::new("/usr/bin/ffmpeg.exe")
    });
    assert!(r.is_ok(), "got: {r:?}");
}

#[test]
fn ensure_download_tool_returns_the_typed_error_when_absent_everywhere() {
    let path = std::env::join_paths(["/a", "/b", "/c"].map(PathBuf::from)).unwrap();
    let r = ensure_download_tool_in_path(both(), &path, |_| false);
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
}

#[test]
fn ensure_download_tool_returns_the_typed_error_for_empty_path() {
    // join_paths can't produce an empty value on every platform
    // (Windows allows it, Unix doesn't), so build directly.
    let path = OsString::new();
    let r = ensure_download_tool_in_path(both(), &path, |_| true);
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
}

#[test]
fn bundled_alone_emits_just_the_bundled_dir() {
    let bundled = PathBuf::from("/bundle/bin");
    let got = compose_anicli_path(Some(&bundled), None, None);
    let parts = split(&got);
    // Bundled first, then the FALLBACK_PATH components.
    assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
    let fallback: Vec<PathBuf> = std::env::split_paths(OsStr::new(FALLBACK_PATH)).collect();
    assert_eq!(&parts[1..], fallback.as_slice());
}

// --- windows_env_passthrough ------------------------------------
//
// Reproduces the Windows-only failure where `cmd.env_clear()`
// stripped the OS env vars Git Bash needs to set up its `/tmp`
// mount and load core DLLs. Without these, the first ani-cli
// spawn after backend startup hits `mktemp: ... '/tmp/...':
// Permission denied`, the script's variables go empty, paths
// collapse to `/`, and the user sees a "Network trouble" toast
// because the gibberish stdout misclassifies on the frontend.
//
// The helper is a pure (env, key-list) → (key, value) pairs
// function so these tests can run on Linux CI too.

use std::collections::HashMap;

fn env_reader(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<OsString> {
    move |k| map.get(k).map(|v| OsString::from(*v))
}

#[test]
fn windows_passthrough_returns_all_keys_when_all_present() {
    // Happy path: every documented var is set in the parent env;
    // the helper forwards all of them, in the documented order so
    // tests downstream can assert on positional equality.
    let env = env_reader(HashMap::from([
        ("TMP", r"C:\Users\joe\AppData\Local\Temp"),
        ("TEMP", r"C:\Users\joe\AppData\Local\Temp"),
        ("SYSTEMROOT", r"C:\Windows"),
        ("USERPROFILE", r"C:\Users\joe"),
        ("LOCALAPPDATA", r"C:\Users\joe\AppData\Local"),
        ("APPDATA", r"C:\Users\joe\AppData\Roaming"),
        ("COMSPEC", r"C:\Windows\System32\cmd.exe"),
        ("WINDIR", r"C:\Windows"),
    ]));
    let got = windows_env_passthrough(&env);
    let names: Vec<&'static str> = got.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        names,
        vec![
            "TMP",
            "TEMP",
            "SYSTEMROOT",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "COMSPEC",
            "WINDIR",
        ]
    );
    assert_eq!(
        got.iter().find(|(k, _)| *k == "SYSTEMROOT").unwrap().1,
        OsString::from(r"C:\Windows")
    );
}

#[test]
fn windows_passthrough_skips_missing_keys_preserving_order() {
    // Partial env: scoop-style minimal user shells often have TMP
    // but no APPDATA, or vice versa. Forward what's there; don't
    // emit a key with an empty value masquerading as "set" because
    // the `env_clear()`-then-restore design is supposed to be
    // transparent to anything we don't explicitly carry over.
    let env = env_reader(HashMap::from([
        ("TMP", r"C:\Temp"),
        ("SYSTEMROOT", r"C:\Windows"),
        ("WINDIR", r"C:\Windows"),
    ]));
    let got = windows_env_passthrough(&env);
    let names: Vec<&'static str> = got.iter().map(|(k, _)| *k).collect();
    assert_eq!(names, vec!["TMP", "SYSTEMROOT", "WINDIR"]);
}

#[test]
fn windows_passthrough_returns_empty_when_no_keys_present() {
    // Pathological but valid: a process spawned with a fully
    // scrubbed env. The helper emits nothing — the spawn site
    // never calls cmd.env() with absent values, which is the same
    // shape we'd get if we hadn't wrapped them at all.
    let env = env_reader(HashMap::new());
    let got = windows_env_passthrough(&env);
    assert!(got.is_empty(), "got: {got:?}");
}

#[test]
fn windows_passthrough_forwards_empty_string_values() {
    // Windows env API distinguishes empty string from missing —
    // `set FOO=` leaves FOO defined but empty. Git Bash relies on
    // this for some MSYS-mode flags; clobbering them with "drop
    // when empty" semantics would silently change behaviour.
    let env = env_reader(HashMap::from([("TMP", "")]));
    let got = windows_env_passthrough(&env);
    assert_eq!(got, vec![("TMP", OsString::new())]);
}

#[test]
fn windows_passthrough_keys_are_the_documented_set() {
    // Pin the canonical key list so a future refactor can't
    // silently drop one. If you intentionally add or remove a
    // key from WINDOWS_ENV_PASSTHROUGH_KEYS, update this list
    // and write a one-line note in the PR explaining why.
    assert_eq!(
        WINDOWS_ENV_PASSTHROUGH_KEYS,
        &[
            "TMP",
            "TEMP",
            "SYSTEMROOT",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "COMSPEC",
            "WINDIR",
        ]
    );
}

#[test]
fn shim_bin_is_appended_after_every_composed_entry() {
    let inherited = join(&["/usr/bin", "/bin"]);
    let composed = compose_anicli_path(Some(&PathBuf::from("/bundle/bin")), None, Some(&inherited));
    let got = append_shim_bin(composed, Some(&PathBuf::from("/cache/botan-shim")));
    let parts = split(&got);
    assert_eq!(parts.len(), 4);
    assert_eq!(
        parts.last(),
        Some(&PathBuf::from("/cache/botan-shim")),
        "the shim dir must lose to any real botan earlier on PATH"
    );
    assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
}

#[test]
fn no_shim_bin_returns_the_composed_path_unchanged() {
    let inherited = join(&["/usr/bin", "/bin"]);
    let composed = compose_anicli_path(None, None, Some(&inherited));
    let got = append_shim_bin(composed.clone(), None);
    assert_eq!(got, composed);
}

#[test]
fn ensure_download_tool_accepts_ytdlp_when_ffmpeg_is_absent() {
    // ani-cli 4.15's download path runs with either tool
    // (dep_ch_failover "yt-dlp,ffmpeg"); the preflight must not block
    // a yt-dlp-only setup the CLI itself would serve.
    let path = join(&["/a", "/b"]);
    let tool = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let r = ensure_download_tool_in_path(both(), &path, |p| p == PathBuf::from("/b").join(tool));
    assert!(r.is_ok(), "got: {r:?}");
}

#[test]
fn ensure_download_tool_errors_when_both_tools_are_absent() {
    let path = join(&["/a", "/b"]);
    let r = ensure_download_tool_in_path(both(), &path, |_| false);
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
}

#[test]
fn ensure_download_tool_still_accepts_ffmpeg_alone() {
    let path = join(&["/a"]);
    let tool = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let r = ensure_download_tool_in_path(both(), &path, |p| p == PathBuf::from("/a").join(tool));
    assert!(r.is_ok(), "got: {r:?}");
}

#[test]
fn ensure_download_tool_rejects_ytdlp_when_the_script_is_ffmpeg_only() {
    // The full gate: a stale pre-4.15 script's name set must fail the
    // preflight on a machine that has yt-dlp but no ffmpeg, so the
    // typed modal shows instead of a mid-download scraper death.
    let names = download_tool_names(r#"download) dep_ch "ffmpeg" "aria2c" ;;"#);
    let path = join(&["/a", "/b"]);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let r = ensure_download_tool_in_path(names, &path, |p| p == PathBuf::from("/b").join(ytdlp));
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
}

#[test]
fn download_tool_names_include_ytdlp_for_the_4_15_failover_line() {
    // 4.15's download mode accepts either tool.
    let script = r#"case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'
        dep_ch "aria2c"
        ;;
esac"#;
    let names = download_tool_names(script);
    let (ytdlp, ffmpeg) = if cfg!(windows) {
        ("yt-dlp.exe", "ffmpeg.exe")
    } else {
        ("yt-dlp", "ffmpeg")
    };
    assert!(names.contains(&ytdlp), "got: {names:?}");
    assert!(names.contains(&ffmpeg), "got: {names:?}");
}

#[test]
fn download_tool_names_are_ffmpeg_only_for_the_pre_4_15_script() {
    // A stale cache running pre-4.15 hard-requires ffmpeg
    // (`dep_ch "ffmpeg" "aria2c"`); accepting yt-dlp alone there
    // would pass the preflight and then die inside the spawn.
    let script = r#"case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let (ytdlp, ffmpeg) = if cfg!(windows) {
        ("yt-dlp.exe", "ffmpeg.exe")
    } else {
        ("yt-dlp", "ffmpeg")
    };
    assert!(!names.contains(&ytdlp), "got: {names:?}");
    assert!(names.contains(&ffmpeg), "got: {names:?}");
}

#[test]
fn download_tool_names_require_an_actual_invocation() {
    // Mentions that aren't the failover CALL — a no-op builtin's
    // argument, a quoted diagnostic, an assignment — grant nothing:
    // the script's real download path still requires ffmpeg.
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    for decoy in [
        ": dep_ch_failover \"yt-dlp,ffmpeg\"",
        "echo 'dep_ch_failover \"yt-dlp,ffmpeg\"'",
        "msg=dep_ch_failover_\"yt-dlp,ffmpeg\"",
    ] {
        let script = format!("#!/bin/sh\n{decoy}\ndownload) dep_ch \"ffmpeg\" ;;\n");
        let names = download_tool_names(&script);
        assert!(
            !names.contains(&ytdlp),
            "decoy must not grant yt-dlp: {decoy}"
        );
    }
}

#[test]
fn download_tool_names_scope_the_probe_to_the_download_branch() {
    // A customized or self-updated script may invoke this exact
    // failover in an unrelated helper while its download branch
    // still hard-requires ffmpeg. Only the `download)` arm that
    // governs -d mode speaks for download capability; a yt-dlp-only
    // machine passing preflight against this script would die inside
    // ani-cli with the generic scraper error.
    let script = r#"#!/bin/sh
pick_muxer() {
    dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
}
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "out-of-branch invocation must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_close_the_arm_at_inline_terminators() {
    // Valid shell may place the next case pattern right after the
    // terminator on the same line. The download arm ends at the
    // `;;` itself, not at end-of-line: an invocation living in the
    // unrelated next arm must not read as download capability while
    // the download arm still hard-requires ffmpeg.
    let script = r#"#!/bin/sh
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;; other)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "next-arm invocation after an inline ;; must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_treat_quoted_terminators_as_data() {
    // `;;` appearing as quoted command data inside the download arm
    // is not a case terminator: closing the arm on it makes the probe
    // miss the real failover call below and blocks a yt-dlp-only
    // machine that the active script fully supports.
    let script = r#"#!/bin/sh
case "$player_function" in
    download)
        printf '%s' ';;'
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'
        ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        names.contains(&ytdlp),
        "a quoted ;; must not close the download arm: {names:?}"
    );
}

#[test]
fn download_tool_names_ignore_comment_tails() {
    // The mirror image: an arm "opened" inside a trailing comment is
    // not an arm, so an invocation after it grants nothing while the
    // real download branch still hard-requires ffmpeg.
    let script = r#"#!/bin/sh
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac
true # ;; download)
dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a comment-tail download) must not open the arm: {names:?}"
    );
}

#[test]
fn download_tool_names_require_the_player_function_case() {
    // An unrelated case statement may legitimately carry a download)
    // arm — a CLI-flag dispatcher, a mode helper — and even call the
    // failover there. Only the case switching on "$player_function"
    // governs -d mode's dependencies; granting on any arm named
    // download) passes a yt-dlp-only preflight the real download
    // branch will fail.
    let script = r#"#!/bin/sh
case "$1" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        ;;
esac
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "an unrelated case's download arm must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_treat_multiline_strings_as_opaque() {
    // A help string may quote the real capability block verbatim
    // across several lines. Text inside a still-open string is not
    // executable shell: it can neither stand up a case owner nor
    // invoke the failover, or a quoted usage example would enable
    // yt-dlp-only downloads the real dependency arm refuses.
    let script = r#"#!/bin/sh
usage='
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null
        ;;
esac
'
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a quoted capability block must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_treat_heredoc_bodies_as_text() {
    // A usage() heredoc may carry an example capability block. Its
    // body is data fed to cat, not executable shell: it can neither
    // stand up a case owner nor invoke the failover while the real
    // download branch still hard-requires ffmpeg.
    let script = r#"#!/bin/sh
usage() {
    cat <<'EOF'
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null
        ;;
esac
EOF
}
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a heredoc capability block must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_resume_after_heredoc_terminators() {
    // The scan resumes as executable shell after each terminator
    // form — bare, tab-stripped (<<-), quoted, and escaped
    // delimiters — and never opens a heredoc for a herestring or a
    // bare << with no word.
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    for (open, close) in [
        ("<<EOF", "EOF"),
        ("<<-EOF", "\tEOF"),
        ("<< \"EOF\"", "EOF"),
        ("<<\\EOF", "EOF"),
        ("<<< herestring", ""),
        ("<<", ""),
    ] {
        let script = format!(
            "#!/bin/sh\ncat {open}\nplain text\n{close}\ncase \"$player_function\" in\n    download) dep_ch_failover \"yt-dlp,ffmpeg\" ;;\nesac\n"
        );
        assert!(
            download_tool_names(&script).contains(&ytdlp),
            "scan must resume after {open}"
        );
    }
}

#[test]
fn download_tool_names_require_an_expanding_owner() {
    // A single-quoted subject switches on the literal text
    // '$player_function', never the variable: such a dead or
    // documentary case cannot own the download branch even when it
    // carries the failover call.
    let script = r#"#!/bin/sh
case '$player_function' in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        ;;
esac
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a literal-subject case must not own the download branch: {names:?}"
    );
}

#[test]
fn download_tool_names_skip_tab_separated_heredoc_delimiters() {
    // sh accepts a tab between << and the delimiter word; the body
    // is still data, not executable shell.
    let script = "#!/bin/sh\ncat <<\tEOF\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null\n        ;;\nesac\nEOF\ncase \"$player_function\" in\n    download) dep_ch \"ffmpeg\" \"aria2c\" ;;\nesac\n";
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a tab-separated heredoc body must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_require_an_identifier_boundary_on_the_owner() {
    // $player_function_backup is a different variable: the unbraced
    // owner match must stop at a shell identifier boundary, or a
    // longer-named case steals ownership it can never exercise.
    let script = r#"#!/bin/sh
case "$player_function_backup" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        ;;
esac
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a longer identifier must not own the download branch: {names:?}"
    );
}

#[test]
fn download_tool_names_start_comments_after_operators() {
    // A # right after a control operator starts a comment even with
    // no whitespace between them; a ;; and arm inside that comment
    // are ignored text, not a real arm.
    let script = r#"#!/bin/sh
case "$player_function" in
    play) dep_ch "ffmpeg";# ;; download) dep_ch_failover "yt-dlp,ffmpeg"
        ;;
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a post-operator comment must not open an arm: {names:?}"
    );
}

#[test]
fn download_tool_names_pop_ownership_at_inline_esac() {
    // esac can share a line with the next statement: the completed
    // player-function case must be popped at that boundary and the
    // unrelated case after the semicolon seen, or a later download)
    // arm is evaluated under a stale owner.
    let script = r#"#!/bin/sh
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac; case "$other" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a stale owner past an inline esac must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_ignore_function_bodies() {
    // A defined-but-never-called helper is not the script's active
    // download flow: its capability block must not grant while the
    // top-level dependency branch still hard-requires ffmpeg. The
    // real 4.15 dep check is top-level, which the repo-script pin
    // keeps enforced from the accepting side.
    let script = r#"#!/bin/sh
unused_helper() {
    case "$player_function" in
        download)
            dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
            ;;
    esac
}
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a function body must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_track_nested_cases_inside_the_arm() {
    // An invocation inside a nested case is conditional — a
    // legacy-only inner arm must not grant while the download arm's
    // own flow still hard-requires ffmpeg…
    let conditional = r#"#!/bin/sh
case "$player_function" in
    download)
        case "$legacy_mode" in
            yes)
                dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
                ;;
        esac
        dep_ch "ffmpeg" "aria2c"
        ;;
esac"#;
    // …while an invocation at the arm's own level after a nested
    // case has closed is unconditional and must still grant: the
    // inner arm's terminator ends the inner arm, not the download
    // arm.
    let unconditional = r#"#!/bin/sh
case "$player_function" in
    download)
        case "$subs" in
            hard) : ;;
        esac
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'
        ;;
esac"#;
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !download_tool_names(conditional).contains(&ytdlp),
        "a nested-arm invocation must not grant yt-dlp"
    );
    assert!(
        download_tool_names(unconditional).contains(&ytdlp),
        "an at-level invocation after a nested case must still grant"
    );
}

#[test]
fn download_tool_names_end_function_skipping_at_redirected_braces() {
    // POSIX permits redirects on a function's closing compound
    // command; the body ends there all the same, and the real
    // top-level capability check after it must still be seen — or a
    // capable script is misread as ffmpeg-only and yt-dlp-only
    // hosts get blocked by the modal.
    let script = r#"#!/bin/sh
quiet_helper() {
    :
} >/dev/null 2>&1
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'
        ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        names.contains(&ytdlp),
        "a redirected closing brace must end the function skip: {names:?}"
    );
}

#[test]
fn download_tool_names_weigh_the_whole_arm() {
    // A failover call followed by an unconditional hard ffmpeg
    // requirement later in the same arm means ffmpeg is still
    // required: the verdict belongs to the complete arm, not the
    // first invocation seen.
    let script = r#"#!/bin/sh
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        dep_ch "ffmpeg" "aria2c"
        ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a later hard ffmpeg requirement in the arm must veto yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_close_redirected_one_line_definitions() {
    // A one-line helper with redirects after its closing brace is a
    // complete definition; the scan must resume after it or the
    // real capability check below is never seen.
    let script = r#"#!/bin/sh
helper() { :; } >/dev/null 2>&1
case "$player_function" in
    download) dep_ch_failover "yt-dlp,ffmpeg" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        names.contains(&ytdlp),
        "a redirected one-line definition must not wedge the skip: {names:?}"
    );
}

#[test]
fn download_tool_names_veto_on_any_hard_ffmpeg_form_or_depth() {
    // The hard-requirement veto is conservative in both dimensions:
    // the single-quoted dep_ch 'ffmpeg' form counts, and so does a
    // requirement inside a nested platform case — a path that may
    // apply still requires ffmpeg, even though a nested invocation
    // never grants.
    let single_quoted = r#"#!/bin/sh
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        dep_ch 'ffmpeg'
        ;;
esac"#;
    let nested = r#"#!/bin/sh
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        case "$(uname)" in
            Darwin) dep_ch "ffmpeg" ;;
        esac
        ;;
esac"#;
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !download_tool_names(single_quoted).contains(&ytdlp),
        "a single-quoted hard ffmpeg check must veto"
    );
    assert!(
        !download_tool_names(nested).contains(&ytdlp),
        "a nested hard ffmpeg check must veto"
    );
}

#[test]
fn download_tool_names_decide_open_arms_at_every_close_form() {
    // The arm's verdict lands however the arm ends: at the owner
    // esac when the last arm has no terminator, and at end of input
    // when a truncated script leaves the arm open.
    let last_arm_no_terminator = r#"#!/bin/sh
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null
esac"#;
    let truncated = r#"#!/bin/sh
case "$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null"#;
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        download_tool_names(last_arm_no_terminator).contains(&ytdlp),
        "the owner esac closes and decides the arm"
    );
    assert!(
        download_tool_names(truncated).contains(&ytdlp),
        "end of input closes and decides the arm"
    );
}

#[test]
fn download_tool_names_skip_split_line_definitions() {
    // A definition whose opening brace sits on the next line is
    // still a definition: the body is awaited, skipped, and closed
    // at its column-0 brace.
    let script = r#"#!/bin/sh
helper()
{
    dep_ch_failover "yt-dlp,ffmpeg" >/dev/null
}
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "a split-line definition's body must not grant: {names:?}"
    );
}

#[test]
fn download_tool_names_parse_composite_heredoc_delimiters() {
    // A heredoc delimiter may be assembled from quoted and unquoted
    // fragments — <<'E'OF names EOF after quote removal. Stopping at
    // the first fragment fails both ways: the skip can overrun the
    // real terminator and hide a genuine capability check…
    let real_check_after = r#"#!/bin/sh
cat <<'E'OF
documentation text
EOF
case "$player_function" in
    download) dep_ch_failover "yt-dlp,ffmpeg" ;;
esac"#;
    // …or resume early inside the body when a data line matches the
    // truncated fragment, mistaking a documented example for the
    // active flow.
    let example_inside = r#"#!/bin/sh
cat <<'E'OF
E
case "$player_function" in
    download) dep_ch_failover "yt-dlp,ffmpeg" ;;
esac
EOF
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        download_tool_names(real_check_after).contains(&ytdlp),
        "the skip must end at the full composite delimiter"
    );
    assert!(
        !download_tool_names(example_inside).contains(&ytdlp),
        "a data line matching a fragment must not resume the scan"
    );
}

#[test]
fn download_tool_names_keep_heredoc_data_out_of_function_detection() {
    // A help heredoc may document a function at column 0 without a
    // matching closer. That line is data: it must not open a body
    // skip that swallows the real capability check after the
    // terminator.
    let script = r#"#!/bin/sh
cat <<'EOF'
example() {
    documented body with no closer
EOF
case "$player_function" in
    download) dep_ch_failover "yt-dlp,ffmpeg" ;;
esac"#;
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        names.contains(&ytdlp),
        "a documented definition inside a heredoc must not wedge the skip: {names:?}"
    );
}

#[test]
fn download_tool_names_ignore_definitions_inside_carried_strings() {
    // The mirror of the heredoc case: a multi-line string may quote
    // a column-0 definition (and its closer). Both are data — the
    // skip must neither open there nor close a real body early.
    let quoted_def = r#"#!/bin/sh
usage='
example() {
'
case "$player_function" in
    download) dep_ch_failover "yt-dlp,ffmpeg" ;;
esac"#;
    let quoted_closer = r#"#!/bin/sh
real_helper() {
    msg='
}
'
    dep_ch_failover "yt-dlp,ffmpeg" >/dev/null
}
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        download_tool_names(quoted_def).contains(&ytdlp),
        "a quoted column-0 definition must not open a skip"
    );
    assert!(
        !download_tool_names(quoted_closer).contains(&ytdlp),
        "a quoted column-0 closer must not end a real body"
    );
}

#[test]
fn download_tool_names_close_without_granting_and_skip_escaped_owners() {
    // An arm closed by the owner esac without a terminator and
    // without a failover grants nothing…
    let hard_only_esac_close = r#"#!/bin/sh
case "$player_function" in
    download)
        dep_ch "ffmpeg" "aria2c"
esac"#;
    // …and a backslash-escaped dollar in a subject is literal text,
    // not an expansion, so it cannot own the download branch.
    let escaped_owner = r#"#!/bin/sh
case "\$player_function" in
    download)
        dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || true
        ;;
esac
case "$player_function" in
    download) dep_ch "ffmpeg" "aria2c" ;;
esac"#;
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !download_tool_names(hard_only_esac_close).contains(&ytdlp),
        "an esac-closed arm without a failover grants nothing"
    );
    assert!(
        !download_tool_names(escaped_owner).contains(&ytdlp),
        "an escaped-dollar subject must not own the download branch"
    );
}

#[test]
fn download_tool_names_preserve_empty_heredoc_delimiters() {
    // sh accepts an empty quoted delimiter: the heredoc runs to the
    // first empty line. Its body is data — a documentary capability
    // block inside it must not grant while the real download arm
    // still hard-requires ffmpeg.
    let script = "#!/bin/sh\ncat <<''\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null\n        ;;\nesac\n\ncase \"$player_function\" in\n    download) dep_ch \"ffmpeg\" \"aria2c\" ;;\nesac\n";
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "an empty-delimiter heredoc body must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_ignore_commented_markers() {
    // A stale or customized script that merely MENTIONS the failover
    // in a comment still hard-requires ffmpeg on its executable
    // path; granting yt-dlp on the mention would pass the preflight
    // and die inside the subprocess.
    let script = "#!/bin/sh\n# dep_ch_failover \"yt-dlp,ffmpeg\" — described, not executed\ndownload) dep_ch \"ffmpeg\" \"aria2c\" ;;\n";
    let names = download_tool_names(script);
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        !names.contains(&ytdlp),
        "commented marker must not grant yt-dlp: {names:?}"
    );
}

#[test]
fn download_tool_names_accept_ytdlp_for_the_real_repo_script() {
    // Reality pin: the bundled script must be recognized as
    // yt-dlp-capable, or the relaxed preflight silently degrades back
    // to ffmpeg-only for every fresh install.
    let repo_script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root")
        .join("ani-cli");
    let contents = std::fs::read_to_string(repo_script).expect("read repo ani-cli");
    let ytdlp = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    assert!(
        download_tool_names(&contents).contains(&ytdlp),
        "bundled script not recognized as yt-dlp-capable"
    );
}

proptest::proptest! {
    // Totality + the branch law for the capability probe over
    // arbitrary surrounding contents: ffmpeg is always offered,
    // nothing panics, and an in-branch executable marker grants
    // yt-dlp while the same marker outside any download arm — or
    // commented inside one — is consistent with the scoped probe.
    // The surroundings exclude quote, escape, and heredoc-opening
    // characters: an odd quote or a stray << in the prefix would
    // legitimately swallow the constructed block as string or
    // heredoc data (both are opaque), which the example tests pin
    // rather than fuzzing here.
    #[test]
    fn download_tool_names_never_panics_and_obeys_the_branch(
        prefix in "[ !#-&(-;=-\\[\\]-~\n]{0,100}",
        suffix in "[ !#-&(-;=-\\[\\]-~\n]{0,100}",
        marker_line in proptest::option::of(proptest::prelude::any::<bool>()),
        in_branch in proptest::prelude::any::<bool>(),
        indent in "[ \t]{0,4}",
    ) {
        let marker = r#"dep_ch_failover "yt-dlp,ffmpeg""#;
        let text = match marker_line {
            Some(commented) => {
                let hash = if commented { "# " } else { "" };
                let line = format!("{indent}{hash}{marker} >/dev/null");
                if in_branch {
                    format!(
                        "{prefix}\ncase \"$player_function\" in\ndownload)\n{line}\n;;\nesac\n{suffix}"
                    )
                } else {
                    format!("{prefix}\n{line}\n{suffix}")
                }
            }
            None => format!("{prefix}\n{suffix}"),
        };
        let names = download_tool_names(&text);
        let ytdlp = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
        let ffmpeg = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        proptest::prop_assert!(names.contains(&ffmpeg), "ffmpeg always offered");
        // An executable marker placed inside a download arm always
        // grants; the reverse direction can't be asserted from the
        // pieces (the arbitrary prefix may open its own arm around
        // the marker or contain one outright), so consistency with
        // the scoped probe covers the rest.
        if marker_line == Some(false) && in_branch {
            proptest::prop_assert!(
                names.contains(&ytdlp),
                "in-branch executable marker must grant yt-dlp"
            );
        }
        proptest::prop_assert_eq!(
            names.contains(&ytdlp),
            download_branch_invokes_failover(&text),
            "names must agree with the scoped probe"
        );
    }

    // Total on arbitrary path lists — the predicate must never panic.
    #[test]
    fn ensure_download_tool_never_panics(
        dirs in proptest::collection::vec("[a-zA-Z0-9/_.-]{0,24}", 0..8),
    ) {
        let joined = std::env::join_paths(dirs.iter().map(PathBuf::from)).expect("join");
        let _ = ensure_download_tool_in_path(both(), &joined, |_| false);
    }

    // Either tool dropped into any generated component satisfies the
    // preflight; with no tool anywhere the typed error comes back.
    #[test]
    fn ensure_download_tool_finds_either_tool_in_any_component(
        dirs in proptest::collection::vec("[a-z0-9]{1,10}", 1..6),
        pick in proptest::prelude::any::<proptest::sample::Index>(),
        use_ytdlp in proptest::prelude::any::<bool>(),
    ) {
        let joined = std::env::join_paths(dirs.iter().map(|d| PathBuf::from(format!("/{d}"))))
            .expect("join");
        let idx = pick.index(dirs.len());
        let tool = match (cfg!(windows), use_ytdlp) {
            (true, true) => "yt-dlp.exe",
            (true, false) => "ffmpeg.exe",
            (false, true) => "yt-dlp",
            (false, false) => "ffmpeg",
        };
        let target = PathBuf::from(format!("/{}", dirs[idx])).join(tool);
        let found = ensure_download_tool_in_path(both(), &joined, |p| p == target);
        proptest::prop_assert!(found.is_ok(), "expected Ok, got {found:?}");
        let none = ensure_download_tool_in_path(both(), &joined, |_| false);
        proptest::prop_assert!(matches!(none, Err(AniError::FfmpegMissing)));
    }
}

/// Codex P2 #3665336648 — the hard-ffmpeg veto matched three exact
/// strings, so any other valid shell whitespace between `dep_ch` and
/// its argument slipped past it. On a yt-dlp-only host the preflight
/// would then pass and ani-cli would exit later at its own hard
/// check with a generic scraper failure.
#[test]
fn hard_ffmpeg_veto_survives_double_space() {
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover \"yt-dlp,ffmpeg\"\ndep_ch  \"ffmpeg\" ;;\nesac";
    assert!(
        !download_branch_invokes_failover(text),
        "two spaces before the argument still hard-requires ffmpeg"
    );
}

#[test]
fn hard_ffmpeg_veto_survives_tab() {
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover \"yt-dlp,ffmpeg\"\ndep_ch\t\"ffmpeg\" ;;\nesac";
    assert!(
        !download_branch_invokes_failover(text),
        "a tab before the argument still hard-requires ffmpeg"
    );
}

#[test]
fn hard_ffmpeg_veto_survives_mixed_whitespace_unquoted() {
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover \"yt-dlp,ffmpeg\"\ndep_ch \t ffmpeg ;;\nesac";
    assert!(
        !download_branch_invokes_failover(text),
        "mixed whitespace before an unquoted argument still vetoes"
    );
}

#[test]
fn a_longer_function_name_ending_in_dep_ch_does_not_veto() {
    // `my_dep_ch ffmpeg` is a different command; the veto must key on
    // the `dep_ch` word, not on a suffix match.
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover \"yt-dlp,ffmpeg\"\nmy_dep_ch ffmpeg ;;\nesac";
    assert!(
        download_branch_invokes_failover(text),
        "a distinct command whose name merely ends in dep_ch must not veto"
    );
}

/// Codex P2 #3665557944 — the grant side had the same defect the
/// veto side did: an exact-text match. A script spelling the same
/// call `dep_ch_failover  'yt-dlp,ffmpeg'` accepts yt-dlp just as
/// much, but the preflight would miss the grant and block a
/// yt-dlp-only install behind the missing-ffmpeg modal.
#[test]
fn failover_grant_survives_single_quotes() {
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover 'yt-dlp,ffmpeg' ;;\nesac";
    assert!(
        download_branch_invokes_failover(text),
        "a single-quoted argument is the same failover call"
    );
}

#[test]
fn failover_grant_survives_double_space_and_tab() {
    let two = "case \"$player_function\" in\ndownload) dep_ch_failover  \"yt-dlp,ffmpeg\" ;;\nesac";
    let tab = "case \"$player_function\" in\ndownload) dep_ch_failover\t\"yt-dlp,ffmpeg\" ;;\nesac";
    assert!(
        download_branch_invokes_failover(two),
        "two spaces still grant"
    );
    assert!(download_branch_invokes_failover(tab), "a tab still grants");
}

#[test]
fn failover_grant_survives_an_unquoted_argument() {
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover yt-dlp,ffmpeg ;;\nesac";
    assert!(
        download_branch_invokes_failover(text),
        "an unquoted argument is the same failover call"
    );
}

#[test]
fn a_different_failover_argument_does_not_grant() {
    // The grant is specific to the yt-dlp,ffmpeg pair; a failover
    // over some other toolset says nothing about yt-dlp capability.
    let text = "case \"$player_function\" in\ndownload) dep_ch_failover \"aria2c,ffmpeg\" ;;\nesac";
    assert!(
        !download_branch_invokes_failover(text),
        "a failover over other tools must not grant yt-dlp"
    );
}
