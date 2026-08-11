//! Tests for `crate::anicli::process`. Extracted via `#[path]` so the
//! inline `mod tests { ... }` block doesn't count toward the file's
//! CCN — per `project_crap_inline_test_gotcha`.

use super::*;

/// Serializes tests that mutate process-global env (PATH). The lock
/// keeps a cleared PATH from leaking into concurrently running tests.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn locate_ani_cli_with_no_path_and_no_fallback_errors() {
    let _guard = ENV_LOCK.lock().await;
    // Save and clear $PATH so `which` cannot find ani-cli.
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    let r = locate_ani_cli(None);
    if let Some(p) = saved {
        std::env::set_var("PATH", p);
    }
    assert!(matches!(r, Err(AniError::MissingBinary)));
}

#[cfg(unix)]
#[test]
fn find_in_path_requires_the_execute_bit() {
    let dir = tempfile::tempdir().expect("tmp");
    let plain = dir.path().join("not-executable");
    std::fs::write(&plain, "data").expect("write");
    assert!(!is_executable(&plain));
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&plain).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&plain, perms).expect("chmod");
    assert!(is_executable(&plain));
}
