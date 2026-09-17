//! Reader and writer for the app's watch-history file.
//!
//! Format (TSV, one record per line):
//!     <ep_no>\t<id>\t<title>[\t<watched_at_ms>]
//!
//! The first three columns are the ones the CLI's `update_history`
//! used, and so are the atomic semantics: write to `path.new`, then
//! rename. The tests below and the samples under
//! `tests/fixtures/history/` pin that contract — including
//! byte-identity against a fixture the script's writer produced. The
//! fourth column is the app's own: the moment of the watch that
//! wrote the row, kept beside the row so the recency a resume ranks
//! by is written in the same line as the row it ranks, whatever
//! becomes of the cache's copy of the stamp. A row without a watch
//! behind it — a plain resolve, a row from before the column —
//! carries none and reads and writes as before.
//!
//! Nothing marks that column. The title ran to the end of the line
//! before it existed, tabs of its own included, so what tells a
//! moment apart from a number a title ends with is plausibility: a
//! trailing number is the moment only when it is a millisecond
//! stamp inside the window a watch could have been written in, in
//! the exact decimal the writer emits. `Mobile Suit\t0080` stays
//! one title; `\t1700000000000` is a moment. The limit of that is
//! written out at [`split_moment`], which holds the rule.
//!
//! The two never shared a file after the 5.0 CLI re-keyed its history
//! onto provider slugs; the app keeps its own under its state dir.
//! The format is inherited, not shared.
//!
//! Path resolution lives in [`crate::config::paths::gui_history`].

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// One row of the history file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Episode number the user last watched, in the provider's
    /// numbering. Written after each play — by
    /// `play_native_record::write_history` on a fresh resolve, and
    /// directly by `play::write_history_on_cache_hit` when the
    /// resolution came from cache — so on the next launch the
    /// "Continue Watching" row knows where to resume.
    pub ep_no: String,
    /// The provider's show id — a slug on every row written since the
    /// migration. Older rows carry the retired provider's opaque id
    /// instead, which is why the reverse resolver still has a branch
    /// for an id it cannot derive a search term from.
    pub id: String,
    /// Display title, as the provider's own card gives it. Rows
    /// written before the migration carry a `"(<n> episodes)"` tail
    /// the writer of the day appended; nothing appends one now, and
    /// the frontend strips it where it finds it.
    pub title: String,
    /// The moment of the watch that wrote the row, in milliseconds
    /// since the epoch — written by the watch itself, beside the row,
    /// so a cache that refuses the same stamp cannot leave the row
    /// ranked below a sibling's older one. A row a resolve rewrote
    /// keeps the moment of the watch before it; a row no watch ever
    /// wrote, or one from before the column existed, carries none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watched_at: Option<i64>,
}

/// Parse the entire history file into a `Vec<HistoryEntry>`.
///
/// A missing file returns `Ok(vec![])`. Malformed lines are silently
/// dropped — the script's shell parser does the same when the column count
/// doesn't match.
///
/// # Errors
/// Returns [`AniError::Io`] when the file exists but cannot be read.
pub fn read_all(path: &Path) -> Result<Vec<HistoryEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let body = std::fs::read_to_string(path)?;
    Ok(parse(&body))
}

/// Parse a TSV body into entries. Pure function, no I/O.
#[must_use]
pub fn parse(body: &str) -> Vec<HistoryEntry> {
    body.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let ep_no = parts.next()?.to_string();
            let id = parts.next()?.to_string();
            let rest = parts.next()?;
            if ep_no.is_empty() || id.is_empty() {
                return None;
            }
            let (title, watched_at) = split_moment(rest);
            Some(HistoryEntry {
                ep_no,
                id,
                title: title.to_string(),
                watched_at,
            })
        })
        .collect()
}

/// The earliest moment the fourth column is read as a watch:
/// 2020-01-01T00:00:00Z in milliseconds. Nothing wrote the column
/// before the app existed, so a smaller number at the end of a row
/// is one the title ends with — a year, a season, a count a scraper
/// left on — and belongs to the title.
const WATCHED_AT_FLOOR_MS: i64 = 1_577_836_800_000;

/// The far end of the same window: 2100-01-01T00:00:00Z in
/// milliseconds. Nothing this app writes reaches it either.
const WATCHED_AT_CEILING_MS: i64 = 4_102_444_800_000;

/// Split what follows the id into the title and, when the row
/// carries one, the watch's moment.
///
/// The two are not marked apart. The fourth column was added to a
/// file whose rows the title already ran to the end of, tabs
/// included, and a marker would have to be written into every
/// existing row before it could be read out of one. Plausibility
/// stands in for it: the tail is the moment only when it is the
/// exact decimal [`serialize`] emits for a millisecond stamp inside
/// [`WATCHED_AT_FLOOR_MS`]`..`[`WATCHED_AT_CEILING_MS`]. A padded
/// or signed number is a title's, since nothing here writes one.
///
/// The rule is narrow, not exact. A title that itself ends in a tab
/// and a thirteen-digit number landing inside that window is still
/// read as a stamped row, and the next write of the file makes the
/// split permanent. That shape is accepted because it is a
/// thirteen-digit number in a hundred-year window, while the shape
/// the window protects — `Mobile Suit\t0080` — is what titles
/// actually end in.
fn split_moment(rest: &str) -> (&str, Option<i64>) {
    let Some((title, tail)) = rest.rsplit_once('\t') else {
        return (rest, None);
    };
    match tail.parse::<i64>() {
        Ok(ms)
            if tail == ms.to_string()
                && (WATCHED_AT_FLOOR_MS..WATCHED_AT_CEILING_MS).contains(&ms) =>
        {
            (title, Some(ms))
        }
        _ => (rest, None),
    }
}

/// Serialize entries back to the TSV body. Each line ends with `\n`,
/// including the last one (matches what the script writes via its
/// `printf "%s\t%s\t%s\n"` line in `update_history`).
#[must_use]
pub fn serialize(entries: &[HistoryEntry]) -> String {
    let mut out = String::with_capacity(entries.len() * 64);
    for e in entries {
        out.push_str(&e.ep_no);
        out.push('\t');
        out.push_str(&e.id);
        out.push('\t');
        out.push_str(&e.title);
        if let Some(at) = e.watched_at {
            out.push('\t');
            out.push_str(&at.to_string());
        }
        out.push('\n');
    }
    out
}

/// Insert or update an entry, matching by `id`. If `id` is already in the
/// vector, that entry's `ep_no` and `title` are replaced, and its
/// watched-at moment when the new entry carries one — a plain
/// resolve rewrites a row without unwriting the watch before it;
/// otherwise the new entry is appended. The vector is mutated in
/// place. Mirrors `update_history`'s semantics from the script.
pub fn upsert(entries: &mut Vec<HistoryEntry>, new: HistoryEntry) {
    if let Some(existing) = entries.iter_mut().find(|e| e.id == new.id) {
        existing.ep_no = new.ep_no;
        existing.title = new.title;
        if new.watched_at.is_some() {
            existing.watched_at = new.watched_at;
        }
    } else {
        entries.push(new);
    }
}

/// Remove an entry by id. Returns true if a row was removed.
pub fn remove_by_id(entries: &mut Vec<HistoryEntry>, id: &str) -> bool {
    let before = entries.len();
    entries.retain(|e| e.id != id);
    entries.len() != before
}

/// Atomically write the entire history file. Implemented as `path.new` +
/// rename, exactly as the script's `update_history` does. The `.new`
/// sidecar is unlinked before this function returns successfully (the
/// final `rename` overwrites the original atomically on Unix).
///
/// # Errors
/// Returns [`AniError::Io`] for I/O failures (including write or rename).
pub fn write_atomic(path: &Path, entries: &[HistoryEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let new_path = path.with_extension("new");
    let body = serialize(entries);
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_path)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&new_path, path)?;
    // Belt-and-suspenders: if the rename succeeded the new path is gone,
    // but if a previous run crashed mid-rename a stale `.new` could be
    // hanging around. Best-effort cleanup of any sidecar with a different
    // suffix.
    if new_path.exists() {
        let _ = std::fs::remove_file(&new_path);
    }
    Ok(())
}

/// Convenience: read + upsert + write_atomic in one call. Pure error
/// propagation; the on-disk file is mutated in-place.
///
/// # Errors
/// Returns [`AniError::Io`] on read or write failure.
pub fn upsert_and_write(path: &Path, new: HistoryEntry) -> Result<()> {
    let mut entries = read_all(path)?;
    upsert(&mut entries, new);
    write_atomic(path, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        // Repo root → tests/fixtures/history/.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root above the crate")
            .join("tests/fixtures/history")
    }

    fn sample_entry(id: &str, ep: &str) -> HistoryEntry {
        HistoryEntry {
            ep_no: ep.into(),
            id: id.into(),
            title: format!("Test ({id})"),
            watched_at: None,
        }
    }

    /// A row written by a watch carries the watch's moment beside it,
    /// in the same line, so the recency a resume ranks by cannot be
    /// lost to a store the row was not written to. A row written
    /// before the column existed, or by a plain resolve, carries none
    /// and reads and writes as it always did.
    #[test]
    fn a_row_carries_its_watched_at_beside_it() {
        let body = "5\tabc\tOne Piece\t1700000000000\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].title, "One Piece");
        assert_eq!(v[0].watched_at, Some(1_700_000_000_000));
        assert_eq!(serialize(&v), body, "the column round-trips");
        let legacy = parse("5\tabc\tOne Piece\n");
        assert_eq!(legacy[0].watched_at, None);
        assert_eq!(
            serialize(&legacy),
            "5\tabc\tOne Piece\n",
            "no column when there is no stamp"
        );
        let tab_in_title = parse("5\tabc\tOne\tPiece\n");
        assert_eq!(
            tab_in_title[0].title, "One\tPiece",
            "a tail that is not a stamp stays in the title"
        );
        assert_eq!(tab_in_title[0].watched_at, None);
    }

    /// The window in which a trailing number reads as a watch,
    /// pinned here as literals rather than read from the parser:
    /// 2020-01-01T00:00:00Z and 2100-01-01T00:00:00Z in
    /// milliseconds. A test that borrowed the parser's own bounds
    /// would follow them wherever they moved and assert nothing
    /// about where they are.
    const WINDOW_START_MS: i64 = 1_577_836_800_000;
    const WINDOW_END_MS: i64 = 4_102_444_800_000;

    /// Whether a title's own last tab-separated segment would be
    /// taken for a watch — the one shape the format cannot tell
    /// apart from a stamped row, and so the one the round-trip
    /// property has to step around.
    fn tail_reads_as_a_watch(title: &str) -> bool {
        title.rsplit_once('\t').is_some_and(|(_, tail)| {
            tail.parse::<i64>().is_ok_and(|ms| {
                ms.to_string() == tail && (WINDOW_START_MS..WINDOW_END_MS).contains(&ms)
            })
        })
    }

    /// Everything after the second tab was the title before the
    /// fourth column existed, embedded tabs included, and titles
    /// end in numbers: a mecha series' year, a season count, a
    /// four-digit tail a scraper left behind. Reading any trailing
    /// number as the watch's moment takes such a title apart and
    /// the next rewrite of the file makes that permanent. Only a
    /// number that could be a moment the app itself wrote — inside
    /// the window, in the exact decimal the writer emits — is one.
    #[test]
    fn a_number_a_title_ends_with_is_not_a_watch() {
        let legacy = "5\tabc\tMobile Suit\t0080\n";
        let v = parse(legacy);
        assert_eq!(v[0].title, "Mobile Suit\t0080", "the title keeps its tail");
        assert_eq!(v[0].watched_at, None);
        assert_eq!(serialize(&v), legacy, "and the row is rewritten unchanged");

        for tail in ["42", "2024", "0", "1577836799999", "9999999999999999"] {
            let row = format!("5\tabc\tA Title\t{tail}\n");
            let v = parse(&row);
            assert_eq!(
                v[0].title,
                format!("A Title\t{tail}"),
                "{tail} is outside the window a watch could have been written in"
            );
            assert_eq!(v[0].watched_at, None);
            assert_eq!(serialize(&v), row);
        }

        // In the window but not in the decimal the writer emits: a
        // padded number is a title's, since nothing here writes one.
        let padded = parse("5\tabc\tA Title\t01700000000000\n");
        assert_eq!(padded[0].title, "A Title\t01700000000000");
        assert_eq!(padded[0].watched_at, None);

        // The moments the app does write still read as moments,
        // at the edges of the window as much as in the middle.
        for ms in [WINDOW_START_MS, 1_700_000_000_000, WINDOW_END_MS - 1] {
            let v = parse(&format!("5\tabc\tA Title\t{ms}\n"));
            assert_eq!(v[0].title, "A Title");
            assert_eq!(v[0].watched_at, Some(ms), "{ms} is a moment a watch wrote");
        }
    }

    /// A resolve rewrites the row without a stamp of its own; the
    /// watch that stamped it earlier is not unwritten by that. A
    /// watch that carries a stamp replaces the earlier one.
    #[test]
    fn upsert_keeps_a_rows_stamp_unless_the_new_entry_carries_one() {
        let mut entries = vec![HistoryEntry {
            watched_at: Some(1_000),
            ..sample_entry("abc", "3")
        }];
        upsert(&mut entries, sample_entry("abc", "4"));
        assert_eq!(entries[0].ep_no, "4");
        assert_eq!(
            entries[0].watched_at,
            Some(1_000),
            "a stampless rewrite keeps the stamp"
        );
        upsert(
            &mut entries,
            HistoryEntry {
                watched_at: Some(2_000),
                ..sample_entry("abc", "5")
            },
        );
        assert_eq!(
            entries[0].watched_at,
            Some(2_000),
            "a stamped rewrite replaces it"
        );
    }

    #[test]
    fn parse_empty_yields_empty() {
        assert!(parse("").is_empty());
        assert!(parse("\n").is_empty());
    }

    #[test]
    fn parse_three_columns() {
        let body = "5\tabc\tOne Piece (1100 episodes)\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].ep_no, "5");
        assert_eq!(v[0].id, "abc");
        assert_eq!(v[0].title, "One Piece (1100 episodes)");
    }

    #[test]
    fn parse_skips_malformed_lines() {
        // First line missing tabs; second valid; third missing id.
        let body = "no-tabs-line\n\
                    1\tdef\tValid\n\
                    \t\tno-id\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "def");
    }

    #[test]
    fn parse_preserves_tabs_after_third_column() {
        // Title column may itself contain tabs (rare but possible). Only
        // the first two tabs are field separators.
        let body = "1\tid\ttitle\twith\tmore\ttabs\n";
        let v = parse(body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].title, "title\twith\tmore\ttabs");
    }

    #[test]
    fn serialize_round_trips_through_parse() {
        let entries = vec![
            sample_entry("a", "1"),
            sample_entry("b", "5"),
            sample_entry("c", "12"),
        ];
        let body = serialize(&entries);
        let parsed = parse(&body);
        assert_eq!(parsed, entries);
    }

    #[test]
    fn serialize_ends_every_line_with_newline() {
        let entries = vec![sample_entry("a", "1")];
        let body = serialize(&entries);
        assert!(body.ends_with('\n'));
    }

    #[test]
    fn upsert_appends_when_id_is_new() {
        let mut v = vec![sample_entry("a", "1")];
        upsert(&mut v, sample_entry("b", "2"));
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].id, "b");
    }

    #[test]
    fn upsert_replaces_when_id_exists_and_keeps_position() {
        let mut v = vec![
            sample_entry("a", "1"),
            sample_entry("b", "2"),
            sample_entry("c", "3"),
        ];
        let updated = HistoryEntry {
            ep_no: "99".into(),
            id: "b".into(),
            title: "New Title".into(),
            watched_at: None,
        };
        upsert(&mut v, updated);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].id, "b");
        assert_eq!(v[1].ep_no, "99");
        assert_eq!(v[1].title, "New Title");
    }

    #[test]
    fn remove_by_id_drops_the_matching_row() {
        let mut v = vec![sample_entry("a", "1"), sample_entry("b", "2")];
        assert!(remove_by_id(&mut v, "a"));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "b");
    }

    #[test]
    fn remove_by_id_is_noop_when_id_missing() {
        let mut v = vec![sample_entry("a", "1")];
        assert!(!remove_by_id(&mut v, "missing"));
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn read_all_missing_file_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("does-not-exist");
        let v = read_all(&path).unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn read_all_matches_bash_fixture_multi() {
        let v = read_all(&fixtures_dir().join("multi.tsv")).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].ep_no, "12");
        assert_eq!(v[0].id, "abc123");
        assert_eq!(v[0].title, "Attack on Titan (25 episodes)");
        assert_eq!(v[1].id, "def456");
        assert_eq!(v[2].id, "ghi789");
    }

    #[test]
    fn serialize_byte_identical_to_bash_fixture() {
        // Format contract: the script's update_history writes
        //     printf "%s\t%s\t%s\n" "$ep_no" "$id" "$title"
        // Our serialize() must produce byte-identical output for the same
        // logical entries so a reader of either file sees
        // one coherent history file.
        let entries = vec![
            HistoryEntry {
                ep_no: "12".into(),
                id: "abc123".into(),
                title: "Attack on Titan (25 episodes)".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "3".into(),
                id: "def456".into(),
                title: "Demon Slayer (26 episodes)".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "1".into(),
                id: "ghi789".into(),
                title: "Spy x Family (12 episodes)".into(),
                watched_at: None,
            },
        ];
        let our_bytes = serialize(&entries);
        let bash_bytes = std::fs::read_to_string(fixtures_dir().join("multi.tsv")).unwrap();
        assert_eq!(our_bytes, bash_bytes, "byte-identical with bash output");
    }

    #[test]
    fn write_atomic_round_trips_disk_to_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("history");
        let entries = vec![sample_entry("a", "1"), sample_entry("b", "2")];
        write_atomic(&path, &entries).unwrap();
        let back = read_all(&path).unwrap();
        assert_eq!(back, entries);
        // No .new sidecar lingers.
        assert!(!path.with_extension("new").exists());
    }

    #[test]
    fn write_atomic_creates_parent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested/dir/history");
        let entries = vec![sample_entry("a", "1")];
        write_atomic(&path, &entries).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn upsert_and_write_creates_then_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("history");

        upsert_and_write(&path, sample_entry("a", "1")).unwrap();
        upsert_and_write(&path, sample_entry("b", "2")).unwrap();
        let v1 = read_all(&path).unwrap();
        assert_eq!(v1.len(), 2);

        // Re-upsert "a" with a new ep_no.
        upsert_and_write(
            &path,
            HistoryEntry {
                ep_no: "99".into(),
                id: "a".into(),
                title: "Test (a)".into(),
                watched_at: None,
            },
        )
        .unwrap();
        let v2 = read_all(&path).unwrap();
        assert_eq!(v2.len(), 2);
        assert_eq!(v2[0].id, "a");
        assert_eq!(v2[0].ep_no, "99");
    }

    // — Properties ────────────────────────────────────────────────────
    //
    // The line format is the one the bash script uses, characterized
    // from it and kept deliberately. The files are separate — the GUI
    // owns `<state_dir>/history` and the script keeps its own — so
    // nothing here can make one side's rows vanish from the other's
    // file. What roundtripping (serialize then parse) protects is this
    // module against itself: a serializer and parser that disagree
    // lose the user's own history on the next load, and a format that
    // drifts from the characterized one loses the reason these rules
    // are what they are.
    use proptest::prelude::*;

    /// Generate a single field that's safe to put in a TSV row.
    ///
    /// Excludes `\t` and `\n` because they're the column/row
    /// separators, and `\r` because Rust's `str::lines()` (which
    /// `parse` uses) strips a trailing `\r` from each line as part
    /// of CRLF normalization — so a title containing `\r` wouldn't
    /// roundtrip. The writer here emits pure `\n`, so these
    /// constraints are the ones the format actually imposes.
    fn tsv_field(min_len: usize, max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<char>(), min_len..=max_len)
            .prop_filter("no tabs, newlines, or carriage returns", |chars| {
                chars.iter().all(|c| *c != '\t' && *c != '\n' && *c != '\r')
            })
            .prop_map(|chars| chars.into_iter().collect())
    }

    fn entry_strategy() -> impl Strategy<Value = HistoryEntry> {
        // ep_no and id must be non-empty (parse() drops rows otherwise).
        // title may be empty. A row may carry its watch's moment, or
        // none, as the file holds both. The moment is drawn from the
        // window the column is read in: a number outside it is a
        // title's tail by the format's own rule, so a row carrying
        // one is not a row the writer could have produced.
        (
            tsv_field(1, 8),
            tsv_field(1, 32),
            tsv_field(0, 64),
            proptest::option::of(WINDOW_START_MS..WINDOW_END_MS),
        )
            .prop_map(|(ep_no, id, title, watched_at)| HistoryEntry {
                ep_no,
                id,
                title,
                watched_at,
            })
    }

    proptest! {
        /// `parse(serialize(entries)) == entries` for any well-formed
        /// vector. The format has no escaping, so the property only
        /// holds when fields are TSV-clean (no embedded tabs/newlines)
        /// — which `serialize` is what guarantees.
        #[test]
        fn parse_serialize_roundtrip(
            entries in proptest::collection::vec(entry_strategy(), 0..16),
        ) {
            let body = serialize(&entries);
            let parsed = parse(&body);
            prop_assert_eq!(entries, parsed);
        }

        /// A legacy title carries tabs of its own, and the format
        /// has no escape for them: what protects such a row is that
        /// a trailing number only reads as a watch inside the
        /// window. For any row whose title's tail is not itself a
        /// moment, title and moment both survive a write and a read
        /// whole — however many tabs the title holds.
        #[test]
        fn a_tabbed_title_survives_the_round_trip(
            ep_no in tsv_field(1, 8),
            id in tsv_field(1, 32),
            segments in proptest::collection::vec(tsv_field(0, 12), 1..4),
            watched_at in proptest::option::of(WINDOW_START_MS..WINDOW_END_MS),
        ) {
            let title = segments.join("\t");
            // A stampless row whose title already ends in something
            // the format reads as a moment is the ambiguity the
            // window narrows but cannot close; it is not a claim
            // this property makes.
            prop_assume!(watched_at.is_some() || !tail_reads_as_a_watch(&title));
            let entry = HistoryEntry { ep_no, id, title, watched_at };
            let parsed = parse(&serialize(std::slice::from_ref(&entry)));
            prop_assert_eq!(parsed, vec![entry]);
        }

        /// The other side of the same rule: a title ending in a
        /// number that no watch could have written keeps it, tab
        /// included, and the row reads as the stampless row it is.
        #[test]
        fn a_number_outside_the_window_stays_in_the_title(
            head in tsv_field(0, 16),
            tail in prop_oneof![0i64..WINDOW_START_MS, WINDOW_END_MS..i64::MAX],
        ) {
            let title = format!("{head}\t{tail}");
            let row = format!("5\tabc\t{title}\n");
            let parsed = parse(&row);
            prop_assert_eq!(&parsed[0].title, &title);
            prop_assert_eq!(parsed[0].watched_at, None);
            prop_assert_eq!(serialize(&parsed), row);
        }

        /// `upsert` is idempotent on the same entry: applying it twice
        /// produces the same vector as applying it once. Every writer
        /// here relies on it — replaying the same play action, or
        /// marking an episode watched twice, mustn't multiply rows.
        #[test]
        fn upsert_is_idempotent(
            initial in proptest::collection::vec(entry_strategy(), 0..8),
            new in entry_strategy(),
        ) {
            let mut once = initial.clone();
            upsert(&mut once, new.clone());
            let mut twice = once.clone();
            upsert(&mut twice, new);
            prop_assert_eq!(once, twice);
        }

        /// `remove_by_id` followed by `upsert` of the same id leaves
        /// the entry present exactly once, with the new ep_no/title.
        #[test]
        fn remove_then_upsert_yields_single_entry(
            initial in proptest::collection::vec(entry_strategy(), 0..8),
            target in entry_strategy(),
        ) {
            let mut entries = initial;
            remove_by_id(&mut entries, &target.id);
            upsert(&mut entries, target.clone());
            let matches: Vec<&HistoryEntry> = entries.iter().filter(|e| e.id == target.id).collect();
            prop_assert_eq!(matches.len(), 1);
            prop_assert_eq!(matches[0], &target);
        }
    }
}
