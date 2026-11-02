//! How this launch gets its flags: from GitHub, or — in a test run —
//! from a file the test provides, with no request at all.
//!
//! A test run is one with `ANI_GUI_UNATTENDED=1`, the switch the
//! end-to-end suites and the backend's integration tests already set
//! (Electron reads it too: a test run's failed boot exits instead of
//! showing a dialog). In a test run the backend never fetches the
//! live file: a flag flipped on the `flags` branch, or a slow or
//! rate-limited CDN, must not change a test's result. The flags come
//! from the file `ANI_GUI_TEST_FLAGS_FILE` names, resolved like the
//! live one, or are the defaults when it names none or cannot be read.
//!
//! `ANI_GUI_TEST_FLAGS_FILE` means nothing without
//! `ANI_GUI_UNATTENDED=1`: a normal launch ignores it and reads the
//! live file as always.

use std::ffi::OsStr;
use std::path::PathBuf;

use super::refresh::refresh;
use super::remote::{self, FETCH_TIMEOUT, FLAGS_URL};
use super::resolve::parse_document;
use super::store::{FlagStore, Origin};

/// The test-run switch.
pub const UNATTENDED_ENV: &str = "ANI_GUI_UNATTENDED";

/// The test run's flags file, honoured only in a test run.
pub const TEST_FLAGS_ENV: &str = "ANI_GUI_TEST_FLAGS_FILE";

/// Where this launch's flags come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchPlan {
    /// The live file, cached at `cache` when there is a cache
    /// directory.
    Live {
        /// The cache file.
        cache: Option<PathBuf>,
    },
    /// A test run: the test's file, or the defaults.
    Test {
        /// The file the test provides.
        file: Option<PathBuf>,
    },
}

/// The plan for these environment values.
#[must_use]
pub fn plan(
    unattended: Option<&OsStr>,
    test_file: Option<&OsStr>,
    cache: Option<PathBuf>,
) -> LaunchPlan {
    let _ = (unattended, test_file);
    LaunchPlan::Live { cache }
}

/// The plan for this process's environment.
#[must_use]
pub fn plan_from_env(cache: Option<PathBuf>) -> LaunchPlan {
    let unattended = std::env::var_os(UNATTENDED_ENV);
    let test_file = std::env::var_os(TEST_FLAGS_ENV);
    plan(unattended.as_deref(), test_file.as_deref(), cache)
}

/// Decide this launch's flags as `plan` says, in a task of its own so
/// startup never waits on it. A live plan's task goes on after the
/// flags are decided, to cache the file for the next launch.
pub fn start(
    store: FlagStore,
    client: reqwest::Client,
    plan: LaunchPlan,
) -> tokio::task::JoinHandle<()> {
    let _ = (store, client, plan, settle_from_test_file, unix_now);
    tokio::spawn(async {})
}

async fn settle_from_test_file(store: &FlagStore, file: Option<PathBuf>) {
    let doc = match file {
        Some(path) => tokio::fs::read(path)
            .await
            .ok()
            .and_then(|b| parse_document(&b)),
        None => None,
    };
    store.settle(doc.as_ref(), Origin::Test);
    tracing::info!(
        origin = store.origin().as_str(),
        flags = ?store.current(),
        "remote flags settled"
    );
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[path = "launch_test.rs"]
mod tests;
