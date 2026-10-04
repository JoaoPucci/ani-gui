//! Writers of one history file take turns.

use super::*;

fn entry(id: String) -> HistoryEntry {
    HistoryEntry {
        ep_no: "1".into(),
        id,
        title: "T".into(),
        watched_at: None,
        kitsu_id: None,
    }
}

/// Every write of the history reads the file, changes it and writes it
/// back. Two at once would each write back what they read, and the
/// later one would lose the earlier one's row — or bring back a row a
/// removal in between had taken out.
#[test]
fn concurrent_writes_lose_no_row() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("history");
    let writers: Vec<_> = (0..8)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                for n in 0..25 {
                    upsert_and_write(&path, entry(format!("show-{writer}-{n}"))).expect("write");
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("writer");
    }
    assert_eq!(read_all(&path).expect("rows").len(), 200);
}
