//! The offsets store's file format and locked I/O — split from the
//! translation boundary for the per-file complexity bar. One TSV
//! row per slug: `slug\toffset[\tslot\ttag]`, the optional pair
//! being the last watch's display stamp.

use std::path::{Path, PathBuf};

use crate::app::AppState;

#[path = "anidb_offset_store_format.rs"]
mod format;
pub(super) use format::parse;
use format::serialize;

/// The offsets file: a sibling of the history file, one `slug\toffset` row
/// per stamped show.
pub(super) fn store_path(history_path: &Path) -> PathBuf {
    history_path.with_file_name("ani-gui-offsets")
}

/// The cross-process lock beside the store. A dedicated file rather
/// than the store itself because the atomic rename replaces the
/// store's inode — a lock held on the old inode would exclude nobody
/// writing the new one.
fn lock_path(store: &Path) -> PathBuf {
    store.with_file_name("ani-gui-offsets.lock")
}

/// One store row: the slug's offset plus, when the last native
/// watch landed on a row whose display tag differs from its slot,
/// that (slot, tag) pair — the bridge that lets the history file
/// carry the slot a resume looks up while the read boundary presents
/// the display identity.
///
/// The slot is stored for this app and no other. Nothing outside it
/// reads this file: it sits beside a history the CLI cannot reach,
/// under a directory that differs between the released build and a
/// dev one.
pub(super) struct Row {
    pub(super) slug: String,
    pub(super) offset: u32,
    pub(super) display: Option<(u32, String)>,
}

/// The locked read-merge-write every put shares.
pub(super) fn merge_row(state: &AppState, slug: &str, offset: u32, display: Option<(u32, String)>) {
    let merge = |rows: &mut Vec<Row>| match rows.iter_mut().find(|r| r.slug == slug) {
        Some(row) => {
            row.offset = offset;
            // An offset-only put must not erase the display
            // stamp — every fresh resolve re-stamps the offset,
            // and the last fractional watch has to stay
            // translatable until something replaces it.
            if display.is_some() {
                row.display = display;
            }
        }
        None => rows.push(Row {
            slug: slug.to_string(),
            offset,
            display,
        }),
    };
    if let Err(e) = rewrite(state, merge) {
        tracing::warn!(slug, offset, error = ?e, "anidb offset write failed");
    }
}

/// Drop every row `forget` picks, under the same locks as a put.
pub(super) fn remove_rows(state: &AppState, forget: impl Fn(&Row) -> bool) -> std::io::Result<()> {
    rewrite(state, |rows| rows.retain(|r| !forget(r)))
}

/// Read the store, apply `change`, and write it back atomically, under
/// the process mutex and the cross-process file lock.
fn rewrite(state: &AppState, change: impl FnOnce(&mut Vec<Row>)) -> std::io::Result<()> {
    let path = store_path(&state.history_path);
    let _guard = PUT_LOCK.lock().expect("offset put lock");
    let write = || -> std::io::Result<()> {
        // A fresh profile reaches this write before anything has
        // created the state dir — the history writer only
        // runs afterwards.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // The other app instance's put: an OS lock held across the
        // whole read-merge-rename, released on drop — and by the
        // kernel if this process dies holding it. Taken after the
        // in-process mutex so threads here queue on the cheap lock
        // and only one of them contends the file.
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path(&path))?;
        fs4::FileExt::lock(&lock_file)?;
        // A missing store is an empty one. Any other read failure ends
        // the rewrite: a store that exists but cannot be read rewritten
        // from nothing would lose every show's offset.
        let body = match std::fs::read_to_string(&path) {
            Ok(body) => body,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut rows = parse(&body);
        change(&mut rows);
        // Atomic like the history writer: a concurrent reader sees
        // the full pre- or post-state, never a half-written file.
        let tmp = path.with_extension("new");
        std::fs::write(&tmp, serialize(&rows))?;
        std::fs::rename(&tmp, &path)
    };
    write()
}

/// Serializes every put's read-merge-write sequence within this
/// process: concurrent prefetch resolves would otherwise read the
/// same old file, independently merge their row, and overwrite one
/// another (or consume each other's temp file) — a lost stamp
/// exposes provider numbering on the home rail. Cross-process
/// exclusion — two processes resolving the same history path, whether
/// two instances of one build or a packaged binary run with
/// `ANI_GUI_DEV` set, which lands it on the dev profile's files — is
/// the OS file lock taken inside `put`; this mutex keeps the process's
/// own threads from contending it.
static PUT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
