//! A backend started with `ANI_GUI_TEST_PROFILE_ROOT` keeps its whole
//! profile under that folder and writes nothing anywhere else.
//!
//! The real binary, run as `common::command` runs it, then with every
//! variable a platform resolves a user folder from — `HOME`, the XDG
//! directories, `USERPROFILE`, `APPDATA`, `LOCALAPPDATA` — pointed at
//! a sentinel folder apart from the root. The backend boots, which
//! creates its cache and runs the boot sweeps, and the test then has
//! it write its settings and its history and report its history path
//! and default download folder.
//!
//! What the sentinel proves depends on the platform. On Linux every
//! profile folder is resolved from those variables, and on macOS from
//! `HOME`, so a resolver that ignored the root would write into the
//! sentinel, and the sentinel staying empty is the check. Windows
//! resolves its folders through the known-folder API, which no
//! variable redirects, so a resolver ignoring the root there would
//! write into the user's own profile rather than the sentinel; on that
//! platform the check is what the backend reports and writes under
//! the root, which the assertions below make on every platform.

mod common;

use std::path::{Path, PathBuf};

const SECRET_HEADER: &str = "x-ani-gui-internal-secret";

/// Every file and folder under `dir`, at any depth.
fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(entries(&path));
        }
        found.push(path);
    }
    found
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rooted_backend_writes_only_under_its_root() {
    let root = tempfile::tempdir().expect("root");
    let sentinel = tempfile::tempdir().expect("sentinel");
    let s = sentinel.path();

    let mut cmd = common::command(
        Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        root.path(),
    );
    cmd.env("HOME", s)
        .env("XDG_CACHE_HOME", s.join("cache"))
        .env("XDG_CONFIG_HOME", s.join("config"))
        .env("XDG_STATE_HOME", s.join("state"))
        .env("XDG_DATA_HOME", s.join("data"))
        .env("XDG_DOWNLOAD_DIR", s.join("downloads"))
        .env("USERPROFILE", s)
        .env("APPDATA", s.join("appdata"))
        .env("LOCALAPPDATA", s.join("localappdata"));
    let mut backend = common::start(cmd);
    let api = backend.api_base.clone();
    let secret = backend.secret.clone();
    let client = reqwest::Client::new();

    let get = |route: &str| {
        client
            .get(format!("{api}{route}"))
            .header(SECRET_HEADER, &secret)
            .send()
    };
    let info: serde_json::Value = get("/api/app-info")
        .await
        .expect("app-info")
        .json()
        .await
        .expect("app-info json");
    let downloads: Option<String> = get("/api/download/default-dir")
        .await
        .expect("default dir")
        .json()
        .await
        .expect("default dir json");
    let settings: serde_json::Value = get("/api/settings")
        .await
        .expect("settings")
        .json()
        .await
        .expect("settings json");
    let put = client
        .put(format!("{api}/api/settings"))
        .header(SECRET_HEADER, &secret)
        .json(&settings)
        .send()
        .await
        .expect("put settings");
    let cleared = client
        .delete(format!("{api}/api/history"))
        .header(SECRET_HEADER, &secret)
        .send()
        .await
        .expect("clear history");

    let _ = backend.child.kill();
    let _ = backend.child.wait();

    assert!(put.status().is_success(), "{put:?}");
    assert!(cleared.status().is_success(), "{cleared:?}");
    let history = PathBuf::from(info["history_path"].as_str().unwrap_or_default());
    assert!(
        history.starts_with(root.path()),
        "the history is under the root {}: {}",
        root.path().display(),
        history.display()
    );
    let downloads = PathBuf::from(downloads.unwrap_or_default());
    assert!(
        downloads.starts_with(root.path()),
        "the default download folder is under the root {}: {}",
        root.path().display(),
        downloads.display()
    );
    for written in [
        common::profile::metadata_db(root.path()),
        common::profile::config_file(root.path()),
    ] {
        assert!(written.is_file(), "the backend wrote {}", written.display());
    }
    let escaped = entries(s);
    assert!(
        escaped.is_empty(),
        "the backend wrote outside its root {}: {escaped:#?}",
        root.path().display()
    );
}
