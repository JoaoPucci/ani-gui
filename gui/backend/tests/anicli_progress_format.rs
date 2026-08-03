//! Drift detector for `ani-cli`'s progress lines.
//!
//! The SSE loading overlay (M2g) forwards classified progress lines
//! to the renderer so the user sees what step we're on. 5.0 emits one
//! `anidb.app links fetched` info line — and routes it to STDOUT,
//! where 4.15 sent its per-provider lines to stderr with an explicit
//! 1>&2. We don't own that script — `pystardust/ani-cli` does — and
//! they patch its scrape every few weeks.
//!
//! This test runs the **real** vendored ani-cli through the curl shim
//! and asserts the stdout still carries a line parse_progress_line
//! classifies as LinksFetched. If upstream rewords the message or
//! drops it, this fails loudly — long before users see "Loading…"
//! with no progress text. (The overlay's own switch to reading stdout
//! is tracked with the rest of the GUI's 5.0 adaptation in
//! docs/deferred-work.md.)
//!
//! Linux-only for the same reason as `anicli_run_debug.rs`.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use ani_gui::anicli::parser::{parse_progress_line, strip_ansi, ProgressLine};

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("manifest is two levels deep from repo root")
        .to_path_buf()
}

/// Stage the curl shim under both names 5.0's failover probes, with
/// CURL_FIXTURE_DIR pinned to the repo's anidb fixtures.
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

#[test]
fn ani_cli_emits_links_fetched_progress_lines() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let bin = stage_anidb_shim(tmp.path());
    let hist = tmp.path().join("hist");
    std::fs::create_dir_all(&hist).expect("mkdir hist");

    let ani_cli_path = repo_root().join("ani-cli");
    assert!(ani_cli_path.is_file(), "ani-cli script exists");

    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{system_path}", bin.display());

    // Use the real script via std::process::Command so we can capture
    // its streams verbatim. 5.0 routes progress to stdout.
    let output = Command::new(&ani_cli_path)
        .args(["-S", "1", "-e", "1", "-q", "best", "--", "test"])
        .env_clear()
        .env("PATH", &path)
        .env("HOME", tmp.path())
        .env("ANI_CLI_HIST_DIR", &hist)
        .env("ANI_CLI_PLAYER", "debug")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .output()
        .expect("ani-cli runs");

    assert!(
        output.status.success(),
        "ani-cli exited with status {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = strip_ansi(&output.stdout);
    let parsed: Vec<ProgressLine> = stdout.lines().filter_map(parse_progress_line).collect();

    // Drift contract: at least one links-fetched line must classify.
    // The wording is the load-bearing assertion — if upstream renames
    // it, this fails, and someone updates parse_progress_line()
    // before the SSE overlay regresses to no-text.
    let has_links_fetched = parsed
        .iter()
        .any(|p| matches!(p, ProgressLine::LinksFetched { .. }));
    assert!(
        has_links_fetched,
        "ani-cli stdout no longer carries a classifiable links-fetched line. \
         Either upstream changed the wording (update parse_progress_line) or \
         the fixtures stopped resolving a stream. Captured lines:\n{parsed:#?}"
    );
}
