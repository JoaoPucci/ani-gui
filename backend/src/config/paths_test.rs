//! The test-only profile root: with [`TEST_PROFILE_ROOT_ENV`] set,
//! every directory and file the backend resolves for its profile lies
//! under that one folder, on every platform — whatever the XDG
//! variables, `HOME` or the platform's own folder APIs would say.
//!
//! Extracted via `#[path]` so the cases do not count against the
//! module's own complexity.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use proptest::prelude::*;

use super::*;

const ROOT_ENV: &str = TEST_PROFILE_ROOT_ENV;

/// Every resolver `paths` exposes, by name. A resolver added to the
/// module belongs here too; the spawned-binary check in
/// `tests/profile_root.rs` catches one that is not.
const RESOLVERS: [(&str, fn() -> Option<PathBuf>); 9] = [
    ("config_file", config_file),
    ("cache_dir", cache_dir),
    ("image_cache_dir", image_cache_dir),
    ("flags_cache", flags_cache),
    ("metadata_db", metadata_db),
    ("logs_dir", logs_dir),
    ("state_dir", state_dir),
    ("gui_history", gui_history),
    ("download_dir", download_dir),
];

/// The variables the resolvers read besides the root, each pointed
/// somewhere the root is not, so a resolver that consults one of them
/// first lands outside it.
const OTHER_VARS: [&str; 8] = [
    "HOME",
    "USERPROFILE",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_DOWNLOAD_DIR",
    "ANI_GUI_DEV",
];

/// Set `vars` for the length of `f`, then put back what was there.
fn with_env<T>(vars: &[(&str, Option<OsString>)], f: impl FnOnce() -> T) -> T {
    let _guard = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved: Vec<(&str, Option<OsString>)> = vars
        .iter()
        .map(|(k, _)| (*k, std::env::var_os(k)))
        .collect();
    for (k, v) in vars {
        match v {
            Some(v) => std::env::set_var(k, v),
            None => std::env::remove_var(k),
        }
    }
    let out = f();
    for (k, v) in saved {
        match v {
            Some(v) => std::env::set_var(k, v),
            None => std::env::remove_var(k),
        }
    }
    out
}

/// The environment of a run with the root set and everything else
/// pointed at `elsewhere`.
fn rooted_env(root: &Path, elsewhere: &Path) -> Vec<(&'static str, Option<OsString>)> {
    let mut vars = vec![(ROOT_ENV, Some(root.as_os_str().to_owned()))];
    for k in OTHER_VARS {
        vars.push((k, Some(elsewhere.join(k).into_os_string())));
    }
    vars
}

fn resolved_outside(root: &Path) -> Vec<String> {
    RESOLVERS
        .iter()
        .filter_map(|(name, resolve)| match resolve() {
            Some(p) if p.starts_with(root) && p != root => None,
            other => Some(format!("{name}: {other:?}")),
        })
        .collect()
}

fn a_root() -> impl Strategy<Value = PathBuf> {
    // Names a test harness could hand over: spaces, dots, quotes and
    // non-ASCII included, nested to a few levels.
    prop::collection::vec("[a-zA-Z0-9 ._'é-]{1,12}", 1..4).prop_map(|segments| {
        let mut root = std::env::temp_dir().join("profile-root-prop");
        for s in segments {
            // `.` and `..` are not names; a trimmed segment keeps the
            // root a plain nested folder.
            let s = s.trim_matches('.');
            root.push(if s.is_empty() { "x" } else { s });
        }
        root
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// With the root set, no resolver answers outside it — the XDG
    /// variables, `HOME` and the dev-profile switch notwithstanding.
    #[test]
    fn every_profile_path_resolves_under_the_root(root in a_root()) {
        let elsewhere = std::env::temp_dir().join("profile-root-elsewhere");
        let outside = with_env(&rooted_env(&root, &elsewhere), || resolved_outside(&root));
        prop_assert!(outside.is_empty(), "root {root:?}; outside it: {outside:#?}");
    }
}

/// The kinds stay apart under the root: config, cache, state, logs
/// and downloads are not one folder.
#[test]
fn the_root_keeps_each_kind_in_a_folder_of_its_own() {
    let root = std::env::temp_dir().join("profile-root-kinds");
    let elsewhere = std::env::temp_dir().join("profile-root-elsewhere");
    let dirs = with_env(&rooted_env(&root, &elsewhere), || {
        [
            config_file().and_then(|p| p.parent().map(Path::to_path_buf)),
            cache_dir(),
            state_dir(),
            logs_dir(),
            download_dir(),
        ]
    });
    let dirs: Vec<PathBuf> = dirs.into_iter().map(|d| d.expect("resolves")).collect();
    let unique: std::collections::BTreeSet<&PathBuf> = dirs.iter().collect();
    assert_eq!(unique.len(), dirs.len(), "{dirs:#?}");
    for d in &dirs {
        assert!(d.starts_with(&root), "{d:?} is under {root:?}");
    }
}

/// An empty value is no root: the resolvers answer as they do with
/// the variable unset, rather than relative to the working directory.
#[test]
fn an_empty_root_is_no_root() {
    let elsewhere = std::env::temp_dir().join("profile-root-empty");
    let mut vars = rooted_env(Path::new(""), &elsewhere);
    vars[0] = (ROOT_ENV, Some(OsString::new()));
    let unset = {
        let mut v = vars.clone();
        v[0] = (ROOT_ENV, None);
        with_env(&v, || RESOLVERS.map(|(_, r)| r()))
    };
    let empty = with_env(&vars, || RESOLVERS.map(|(_, r)| r()));
    assert_eq!(empty, unset);
}
