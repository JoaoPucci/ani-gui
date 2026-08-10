//! End-to-end integration test for [`ani_gui::anicli::process::run_debug`].
//!
//! Spawns the real `ani-cli` script with a `curl` shim placed on PATH that
//! returns canned fixtures (the same shim used by `tests/bash/acceptance/`).
//! Verifies that the Rust driver:
//!
//! 1. Spawns the script with the right argv + env scrubbing.
//! 2. Reads stdout, strips ANSI, parses the `Selected link:` block.
//! 3. Returns the resolved URL via `DebugOutput`.
//!
//! Linux-only: `ani-cli` is a POSIX-shell script that depends on bash + a
//! POSIX environment. macOS bash is too old in places to be reliable, and
//! Windows has no native bash at all. The Rust driver is portable; this
//! particular integration test isn't.

#![cfg(target_os = "linux")]

use std::path::PathBuf;

use ani_gui::anicli::process::{run_debug, run_debug_streaming, DebugOptions};

/// Repo root, computed from this test file's location.
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("manifest is two levels deep from repo root")
        .to_path_buf()
}

/// Stage the curl shim on a fresh tmp dir and return that dir so it
/// can be prepended to PATH. The wrapper pins CURL_FIXTURE_DIR to the
/// repo's anidb fixture set (read-only; nothing is copied) and is
/// installed under BOTH `curl` and `curl_firefox135` — ani-cli 5.0
/// prefers curl-impersonate binaries, and the impersonate name comes
/// first in its failover list, so a machine with the real thing would
/// otherwise route test traffic to the live site.
fn stage_anidb_shim(tmp: &std::path::Path) -> PathBuf {
    let bin = tmp.join("bin");
    std::fs::create_dir_all(&bin).expect("mkdir bin");
    let repo = repo_root();
    let body = format!(
        "#!/bin/sh\nexport CURL_FIXTURE_DIR={fixtures}\nexec sh {repo}/tests/bash/helpers/curl_shim.sh \"$@\"\n",
        fixtures = repo.join("tests/fixtures/anidb").display(),
        repo = repo.display(),
    );
    for name in ["curl", "curl_firefox135"] {
        let dst = bin.join(name);
        std::fs::write(&dst, &body).expect("write wrapper shim");
        // `mut` is only used in the cfg(unix) arm; allow(unused_mut)
        // keeps the Windows build clean under -D warnings.
        #[allow(unused_mut)]
        let mut perms = std::fs::metadata(&dst).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
        }
        std::fs::set_permissions(&dst, perms).expect("chmod +x");
    }
    bin
}

#[tokio::test]
async fn run_debug_resolves_stream_url_via_curl_shim() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let bin = stage_anidb_shim(tmp.path());

    let hist = tmp.path().join("hist");
    std::fs::create_dir_all(&hist).expect("mkdir hist");

    // Locate ani-cli at the repo root.
    let ani_cli_path = repo_root().join("ani-cli");
    assert!(ani_cli_path.is_file(), "ani-cli script exists");

    // Compose PATH: tmp/bin (with our curl shim) ahead of the system
    // path so the script's curl resolves to ours.
    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{system_path}", bin.display());

    let opts = DebugOptions {
        ani_cli_path,
        bash_path: None,
        hist_dir: Some(hist),
        timeout: std::time::Duration::from_secs(60),
        path_override: Some(path),
        bundled_bin: None,
        shim_bin: None,
    };

    let out = run_debug(&opts, "test", "1", "best", "sub", 1)
        .await
        .expect("run_debug succeeds");

    assert_eq!(out.selected_url, "https://cdn.example/op/1080/index.m3u8");
    assert!(out
        .all_links
        .iter()
        .any(|l| l == "720p >https://cdn.example/op/720/index.m3u8"));
}

/// `run_search` is intentionally a stub today (see
/// `.planning/cli-contract-deviations.md`). This test pins the contract so
/// a future implementation accidentally not adding tests is loud.
#[tokio::test]
async fn run_search_returns_empty_until_unblocked() {
    let v = ani_gui::anicli::process::run_search("anything", "sub")
        .await
        .expect("stub returns Ok");
    assert!(v.is_empty(), "stub yields no results");
}

/// Streaming variant must call `on_stderr_line` for every stderr line
/// (with ANSI escapes stripped) AND return the same parsed DebugOutput
/// the non-streaming variant returns. The captured lines are what the
/// SSE endpoint forwards to the renderer's loading overlay.
#[tokio::test]
async fn run_debug_streaming_forwards_stderr_lines_in_order() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let bin = stage_anidb_shim(tmp.path());
    let hist = tmp.path().join("hist");
    std::fs::create_dir_all(&hist).expect("mkdir hist");

    let ani_cli_path = repo_root().join("ani-cli");
    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{system_path}", bin.display());

    let opts = DebugOptions {
        ani_cli_path,
        bash_path: None,
        hist_dir: Some(hist),
        timeout: std::time::Duration::from_secs(60),
        path_override: Some(path),
        bundled_bin: None,
        shim_bin: None,
    };

    // Collect every stderr line into a Mutex<Vec> via the callback.
    let captured: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured_for_cb = captured.clone();

    let out = run_debug_streaming(&opts, "test", "1", "best", "sub", 1, move |line| {
        captured_for_cb
            .lock()
            .expect("mutex")
            .push(line.to_string());
    })
    .await
    .expect("run_debug_streaming succeeds");

    // Same DebugOutput contract as the non-streaming variant.
    assert_eq!(out.selected_url, "https://cdn.example/op/1080/index.m3u8");

    // 5.0 routes its info lines to STDOUT (4.15 sent them to stderr
    // with an explicit 1>&2), so the stderr callback sees nothing on
    // the happy path. Pinned as observed; the loading overlay's
    // stdout adaptation is tracked with the rest of the GUI's 5.0
    // work in docs/deferred-work.md.
    let lines = captured.lock().expect("mutex").clone();
    assert!(
        lines.is_empty(),
        "5.0's happy path emits no stderr; got: {lines:#?}"
    );
}

/// The stderr pass-through itself, pinned where 5.0 still writes to
/// stderr: `die` lines. A no-results query makes ani-cli die with
/// "No results found!" — the callback must see it even though the run
/// fails.
#[tokio::test]
async fn run_debug_streaming_forwards_die_lines_from_stderr() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let bin = stage_anidb_shim(tmp.path());
    let hist = tmp.path().join("hist");
    std::fs::create_dir_all(&hist).expect("mkdir hist");

    let opts = DebugOptions {
        ani_cli_path: repo_root().join("ani-cli"),
        bash_path: None,
        hist_dir: Some(hist),
        timeout: std::time::Duration::from_secs(60),
        path_override: Some(format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        )),
        bundled_bin: None,
        shim_bin: None,
    };

    let captured: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured_for_cb = captured.clone();

    let result = run_debug_streaming(&opts, "nohit", "1", "best", "sub", 1, move |line| {
        captured_for_cb
            .lock()
            .expect("mutex")
            .push(line.to_string());
    })
    .await;

    assert!(result.is_err(), "a no-results query fails the run");
    let lines = captured.lock().expect("mutex").clone();
    assert!(
        lines.iter().any(|l| l.contains("No results found")),
        "die line reached the stderr callback: {lines:#?}"
    );
}
