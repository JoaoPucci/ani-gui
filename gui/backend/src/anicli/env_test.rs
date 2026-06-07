//! Tests for `crate::env`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

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
fn ensure_ffmpeg_returns_ok_when_executable_in_first_dir() {
    let path = std::env::join_paths(["/bundle/bin", "/usr/bin"].map(PathBuf::from)).unwrap();
    let r = ensure_ffmpeg_in_path(&path, |p| {
        p == Path::new("/bundle/bin/ffmpeg") || p == Path::new("/bundle/bin/ffmpeg.exe")
    });
    assert!(r.is_ok(), "got: {r:?}");
}

#[test]
fn ensure_ffmpeg_returns_ok_when_executable_in_later_dir() {
    let path = std::env::join_paths(["/no/ffmpeg/here", "/usr/bin"].map(PathBuf::from)).unwrap();
    let r = ensure_ffmpeg_in_path(&path, |p| {
        p == Path::new("/usr/bin/ffmpeg") || p == Path::new("/usr/bin/ffmpeg.exe")
    });
    assert!(r.is_ok(), "got: {r:?}");
}

#[test]
fn ensure_ffmpeg_returns_ffmpeg_missing_when_absent_everywhere() {
    let path = std::env::join_paths(["/a", "/b", "/c"].map(PathBuf::from)).unwrap();
    let r = ensure_ffmpeg_in_path(&path, |_| false);
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got: {r:?}");
}

#[test]
fn ensure_ffmpeg_returns_ffmpeg_missing_for_empty_path() {
    // join_paths can't produce an empty value on every platform
    // (Windows allows it, Unix doesn't), so build directly.
    let path = OsString::new();
    let r = ensure_ffmpeg_in_path(&path, |_| true);
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
