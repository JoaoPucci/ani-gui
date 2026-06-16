//! ani-gui — desktop GUI for the ani-cli anime scraper.
//!
//! This crate is the headless backend of the Electron application. It does
//! three things on the user's machine:
//!
//! 1. Drives the vendored `ani-cli` script (subprocess) to scrape allanime
//!    for search results, episode lists, and resolved stream URLs.
//! 2. Runs a localhost HTTP server that mounts (a) a streaming proxy which
//!    injects `Referer:` and rewrites m3u8 manifests so the embedded
//!    `<video>` + `hls.js` player can fetch segments without CORS pain,
//!    and (b) the API the renderer talks to via plain `fetch()`.
//! 3. Talks to Kitsu (and eventually AniList) for metadata, caches results
//!    in SQLite + images on disk, and reads the shared ani-cli history file.
//!
//! Every listening socket is bound to `127.0.0.1`. The Electron renderer
//! discovers the kernel-assigned port from the `ani-gui-backend` binary's
//! stdout handshake and talks to the API + proxy from there.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod account;
pub mod anicli;
pub mod api;
pub mod app;
pub mod cache;
pub mod commands;
pub mod config;
pub mod error;
pub mod history;
pub mod i18n;
pub mod meta;
pub mod proxy;
pub mod scraper;

pub use error::{AniError, Result};

/// Library version, sourced from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Append a `-dev` marker to a version when running the dev profile, so
/// every surface that shows the version (renderer chip + the
/// diagnostics page) marks a dev build distinctly from an installed
/// release. Pure for testability; see [`display_version`].
fn version_with_dev(version: &str, is_dev: bool) -> String {
    if is_dev {
        format!("{version}-dev")
    } else {
        version.to_string()
    }
}

/// The version string for display surfaces, with `-dev` appended for
/// dev builds — the backend counterpart to the renderer's
/// `versionLabel`. A build is "dev" when it's a debug build (every
/// source-built backend, incl. the standalone dev flow) or `ANI_GUI_DEV`
/// is set, mirroring the data-dir profile in [`config::paths`] so the
/// diagnostics version and the cache dir always agree on dev-vs-release.
#[must_use]
pub fn display_version() -> String {
    let env_dev = std::env::var_os("ANI_GUI_DEV").is_some_and(|v| !v.is_empty());
    version_with_dev(VERSION, env_dev || cfg!(debug_assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_with_dev_appends_suffix_only_in_dev() {
        assert_eq!(version_with_dev("0.9.0", true), "0.9.0-dev");
        assert_eq!(version_with_dev("0.9.0", false), "0.9.0");
    }

    #[test]
    fn version_string_looks_like_semver() {
        // env!() produces a `&'static str`; clippy is right that
        // is_empty() on a const is silly. The check that matters is shape.
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(
            parts.len() >= 2,
            "CARGO_PKG_VERSION should be semver-shaped: {VERSION}"
        );
    }
}
