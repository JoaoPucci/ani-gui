//! Tests for `crate::env`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;
use std::path::PathBuf;

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
fn bundled_alone_emits_just_the_bundled_dir() {
    let bundled = PathBuf::from("/bundle/bin");
    let got = compose_anicli_path(Some(&bundled), None, None);
    let parts = split(&got);
    // Bundled first, then the FALLBACK_PATH components.
    assert_eq!(parts[0], PathBuf::from("/bundle/bin"));
    let fallback: Vec<PathBuf> = std::env::split_paths(OsStr::new(FALLBACK_PATH)).collect();
    assert_eq!(&parts[1..], fallback.as_slice());
}
