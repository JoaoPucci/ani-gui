//! The offsets store's file format and locked I/O — split from the
//! translation boundary for the per-file complexity bar. One TSV
//! row per slug: `slug\toffset[\tslot\ttag]`, the optional pair
//! being the last watch's display stamp.

use std::path::{Path, PathBuf};

use crate::app::AppState;

/// The offsets file: a sibling of `ani-hsts`, one `slug\toffset` row
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
/// that (slot, tag) pair — the bridge that lets ani-hsts speak the
/// CLI's slot numbers while the GUI keeps the display identity.
pub(super) struct Row {
    pub(super) slug: String,
    pub(super) offset: u32,
    pub(super) display: Option<(u32, String)>,
}

pub(super) fn parse(body: &str) -> Vec<Row> {
    body.lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let slug = cols.next()?;
            if slug.is_empty() {
                return None;
            }
            let offset = cols.next()?.trim().parse().ok()?;
            // Optional third+fourth columns; rows written before the
            // display stamp existed have two and parse the same.
            let display = match (cols.next(), cols.next()) {
                (Some(slot), Some(tag)) if !tag.is_empty() => {
                    slot.trim().parse().ok().map(|n| (n, tag.to_string()))
                }
                _ => None,
            };
            Some(Row {
                slug: slug.to_string(),
                offset,
                display,
            })
        })
        .collect()
}

fn serialize(rows: &[Row]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&row.slug);
        out.push('\t');
        out.push_str(&row.offset.to_string());
        if let Some((slot, tag)) = &row.display {
            out.push('\t');
            out.push_str(&slot.to_string());
            out.push('\t');
            out.push_str(tag);
        }
        out.push('\n');
    }
    out
}

/// The locked read-merge-write every mutation shares.
pub(super) fn merge_row(state: &AppState, slug: &str, offset: u32, display: Option<(u32, String)>) {
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
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        let mut rows = parse(&body);
        match rows.iter_mut().find(|r| r.slug == slug) {
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
        }
        // Atomic like the history writer: a concurrent reader sees
        // the full pre- or post-state, never a half-written file.
        let tmp = path.with_extension("new");
        std::fs::write(&tmp, serialize(&rows))?;
        std::fs::rename(&tmp, &path)
    };
    if let Err(e) = write() {
        tracing::warn!(slug, offset, error = ?e, "anidb offset write failed");
    }
}

/// Serializes every put's read-merge-write sequence within this
/// process: concurrent prefetch resolves would otherwise read the
/// same old file, independently merge their row, and overwrite one
/// another (or consume each other's temp file) — a lost stamp
/// exposes provider numbering on the home rail. Cross-process
/// exclusion — the store is shared by the packaged and dev profiles,
/// which can run at once — is the OS file lock taken inside `put`;
/// this mutex keeps the process's own threads from contending it.
static PUT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
