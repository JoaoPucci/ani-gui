//! The watch-order modules' shared test data: the six real franchises of
//! `tests/fixtures/watch-order/runs.json` (*Bleach*, *Attack on Titan*,
//! *JoJo*, *Gintama*, *Fate* seen from *Fate/Zero*, *Monogatari*), as
//! Kitsu served them on 2026-11-02, with made-up progress.

use std::path::PathBuf;

use serde::Deserialize;

use super::{Progress, WatchEntry, WatchRun};

/// One fixture franchise.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FixtureRun {
    pub(crate) key: String,
    #[serde(flatten)]
    pub(crate) run: WatchRun,
    pub(crate) progress: Progress,
}

/// The fixture folder.
pub(crate) fn dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/watch-order")
}

/// Every fixture run.
pub(crate) fn runs() -> Vec<FixtureRun> {
    let raw = std::fs::read(dir().join("runs.json")).expect("runs.json");
    serde_json::from_slice(&raw).expect("runs json")
}

/// The fixture run `key`.
pub(crate) fn run(key: &str) -> WatchRun {
    runs()
        .into_iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no fixture run {key}"))
        .run
}

/// A plain entry: TV, twelve episodes, finished, no art.
pub(crate) fn entry(id: &str, start: Option<&str>) -> WatchEntry {
    WatchEntry {
        id: id.into(),
        title: id.into(),
        subtype: Some("TV".into()),
        start: start.map(Into::into),
        episodes: Some(12),
        status: Some("finished".into()),
        poster: None,
        cover: None,
    }
}

/// The ids of `list`, in order.
pub(crate) fn ids<'a>(list: impl IntoIterator<Item = &'a WatchEntry>) -> Vec<String> {
    list.into_iter().map(|e| e.id.clone()).collect()
}

/// The fixture run `key` as the walk would serve it for `anchor`.
pub(crate) fn payload(key: &str, anchor: &str) -> super::Franchise {
    let r = run(key);
    let parents = r
        .parents
        .get(anchor)
        .map(|p| [(anchor.to_string(), p.clone())].into_iter().collect())
        .unwrap_or_default();
    super::Franchise {
        anchor: anchor.into(),
        name: format!("The {key} franchise"),
        name_from: r.name_id,
        entries: r.entries,
        links: r.links,
        versions: r.versions,
        films: r.films,
        sides: r.sides,
        recaps: r.recaps,
        parents,
        complete: true,
        chain: None,
    }
}

/// The fixture run `key` with its made-up progress.
pub(crate) fn run_with_progress(key: &str) -> (WatchRun, Progress) {
    let r = runs()
        .into_iter()
        .find(|r| r.key == key)
        .expect("a fixture run");
    (r.run, r.progress)
}
