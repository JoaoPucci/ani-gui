//! History commands — `history_list` and `history_clear`.
//!
//! Reads/writes the same `ani-hsts` file the CLI uses, so a user
//! alternating between CLI and GUI sees one coherent history.

use crate::error::Result;
use crate::history::{read_all, write_atomic, HistoryEntry};

/// Returns every history entry as the frontend would render the
/// "Continue Watching" row. Most-recent-first order is the GUI's choice;
/// the on-disk order from the CLI is "append-only with in-place updates",
/// so we return entries in the order they appear on disk and let the
/// frontend reverse if it wants newest-first.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file exists but cannot
/// be read.
pub fn history_list(state: &crate::app::AppState) -> Result<Vec<HistoryEntry>> {
    read_all(&state.history_path)
}

/// Find the history entry (if any) whose allmanga show_id maps to
/// the supplied `kitsu_id`. Walks the on-disk TSV, resolving each
/// entry's `id` through the `(allmanga show_id → kitsu_id)` reverse
/// cache stamped by every successful play. Returns the first match
/// or `None` when:
///   - The history file is missing or empty.
///   - No entry's allmanga id has a cached mapping.
///   - None of the cached mappings equal `kitsu_id`.
///
/// The reverse cache is the same surface Continue Watching uses; if
/// it has no entry for a given allmanga id (the user hasn't played
/// that show through the GUI yet), that history row is skipped here.
/// CLI-only history rows therefore won't surface a Resume affordance
/// until the user plays the show once via the GUI — by design, since
/// otherwise we'd need to round-trip Kitsu search per row.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] when the history file
/// exists but cannot be read; SQLite errors propagate from the
/// reverse-cache lookup.
pub fn history_by_kitsu(
    state: &crate::app::AppState,
    kitsu_id: &str,
) -> Result<Option<HistoryEntry>> {
    if kitsu_id.is_empty() {
        return Ok(None);
    }
    let entries = read_all(&state.history_path)?;
    for entry in entries {
        if let Ok(Some(mapped)) = crate::commands::kitsu::allmanga_kitsu_get(state, &entry.id) {
            if mapped == kitsu_id {
                return Ok(Some(entry));
            }
        }
    }
    Ok(None)
}

/// Truncate the history file to zero length. Mirrors `ani-cli -D`.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file cannot be written.
pub fn history_clear(state: &crate::app::AppState) -> Result<()> {
    write_atomic(&state.history_path, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    fn make_state(history_path: PathBuf) -> AppState {
        AppState {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
            ani_cli_path: PathBuf::from("/x/ani-cli"),
            bash_path: None,
            bundled_bin: None,
            history_path,
            scraper_slots: Arc::new(Semaphore::new(1)),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
        }
    }

    #[test]
    fn list_empty_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_state(tmp.path().join("nope"));
        let v = history_list(&s).unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn by_kitsu_returns_the_matching_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());

        write_atomic(
            &path,
            &[
                HistoryEntry {
                    ep_no: "5".into(),
                    id: "amA".into(),
                    title: "Show A (10 episodes)".into(),
                },
                HistoryEntry {
                    ep_no: "12".into(),
                    id: "amB".into(),
                    title: "Show B (24 episodes)".into(),
                },
            ],
        )
        .unwrap();

        // Prime the (allmanga show_id → kitsu_id) reverse mapping
        // the play path stamps after a successful play.
        crate::commands::kitsu::allmanga_kitsu_put(&s, "amA", "K1").unwrap();
        crate::commands::kitsu::allmanga_kitsu_put(&s, "amB", "K2").unwrap();

        let hit = history_by_kitsu(&s, "K2").unwrap().expect("match");
        assert_eq!(hit.id, "amB");
        assert_eq!(hit.ep_no, "12");
    }

    #[test]
    fn by_kitsu_returns_none_when_no_history_entry_maps_to_id() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());

        write_atomic(
            &path,
            &[HistoryEntry {
                ep_no: "5".into(),
                id: "amA".into(),
                title: "Show A (10 episodes)".into(),
            }],
        )
        .unwrap();
        crate::commands::kitsu::allmanga_kitsu_put(&s, "amA", "K1").unwrap();

        // No history entry maps to K-other.
        assert!(history_by_kitsu(&s, "K-other").unwrap().is_none());
    }

    #[test]
    fn by_kitsu_returns_none_when_history_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_state(tmp.path().join("nope"));
        assert!(history_by_kitsu(&s, "K1").unwrap().is_none());
    }

    // — history_delete ————————————————————————————————————————————
    //
    // Per-row delete operates on the same TSV file the CLI shares.
    // Pins: removes the matching id, preserves others byte-identically,
    // is idempotent (no-op delete of an unknown id returns false), and
    // handles a missing file as "nothing to delete" rather than erroring.

    #[test]
    fn delete_removes_matching_row_and_preserves_others() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());
        write_atomic(
            &path,
            &[
                HistoryEntry {
                    ep_no: "5".into(),
                    id: "amA".into(),
                    title: "Show A".into(),
                },
                HistoryEntry {
                    ep_no: "12".into(),
                    id: "amB".into(),
                    title: "Show B".into(),
                },
                HistoryEntry {
                    ep_no: "3".into(),
                    id: "amC".into(),
                    title: "Show C".into(),
                },
            ],
        )
        .unwrap();

        let removed = history_delete(&s, "amB").unwrap();
        assert!(removed, "delete reports true when a row is removed");

        let after = history_list(&s).unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].id, "amA");
        assert_eq!(after[1].id, "amC");
    }

    #[test]
    fn delete_unknown_id_is_idempotent_no_op() {
        // Per-card double-clicks and bad client retries must be safe.
        // No-op delete returns false; the file stays byte-identical.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());
        write_atomic(
            &path,
            &[HistoryEntry {
                ep_no: "5".into(),
                id: "amA".into(),
                title: "Show A".into(),
            }],
        )
        .unwrap();
        let body_before = std::fs::read_to_string(&path).unwrap();

        let removed = history_delete(&s, "does-not-exist").unwrap();
        assert!(!removed);
        let body_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body_before, body_after);
    }

    #[test]
    fn delete_against_missing_file_returns_false() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_state(tmp.path().join("nope"));
        let removed = history_delete(&s, "amA").unwrap();
        assert!(!removed);
    }

    #[test]
    fn delete_with_empty_id_returns_false() {
        // Defensive: a malformed client call with empty id mustn't
        // accidentally wipe rows with id="" (parser already drops
        // those at read, but pin the contract anyway).
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());
        write_atomic(
            &path,
            &[HistoryEntry {
                ep_no: "5".into(),
                id: "amA".into(),
                title: "Show A".into(),
            }],
        )
        .unwrap();

        let removed = history_delete(&s, "").unwrap();
        assert!(!removed);
        assert_eq!(history_list(&s).unwrap().len(), 1);
    }

    #[test]
    fn list_then_clear_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ani-hsts");
        let s = make_state(path.clone());
        // Pre-populate with a known fixture.
        write_atomic(
            &path,
            &[HistoryEntry {
                ep_no: "5".into(),
                id: "abc".into(),
                title: "T (10 episodes)".into(),
            }],
        )
        .unwrap();

        let listed = history_list(&s).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "abc");

        history_clear(&s).unwrap();
        let after = history_list(&s).unwrap();
        assert!(after.is_empty());
    }
}
