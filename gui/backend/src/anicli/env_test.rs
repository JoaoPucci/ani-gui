//! Tests for `crate::env`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

/// The 4.15 name set, produced through the production probe so every
/// preflight test also exercises the capability gate's positive path.
fn both() -> &'static [&'static str] {
    download_tool_names(r#"dep_ch_failover "yt-dlp,ffmpeg""#)
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
    // Totality + the marker law for the capability probe: yt-dlp is
    // offered exactly when the 4.15 failover marker appears on an
    // executable (non-comment) line, over arbitrary surrounding
    // contents; ffmpeg is always offered and nothing panics.
    #[test]
    fn download_tool_names_never_panics_and_obeys_the_marker(
        prefix in "[ -~\n]{0,100}",
        suffix in "[ -~\n]{0,100}",
        marker_line in proptest::option::of(proptest::prelude::any::<bool>()),
        indent in "[ \t]{0,4}",
    ) {
        let marker = r#"dep_ch_failover "yt-dlp,ffmpeg""#;
        let text = match marker_line {
            Some(commented) => {
                let hash = if commented { "# " } else { "" };
                format!("{prefix}\n{indent}{hash}{marker} >/dev/null\n{suffix}")
            }
            None => format!("{prefix}\n{suffix}"),
        };
        let names = download_tool_names(&text);
        let ytdlp = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
        let ffmpeg = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        proptest::prop_assert!(names.contains(&ffmpeg), "ffmpeg always offered");
        // The arbitrary prefix/suffix may themselves contain an
        // uncommented marker; compute the expectation over the whole
        // constructed text with the same line discipline.
        let expected = text
            .lines()
            .any(|line| line.trim_start().starts_with(marker));
        proptest::prop_assert_eq!(
            names.contains(&ytdlp),
            expected,
            "yt-dlp offered exactly on an executable marker line"
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
