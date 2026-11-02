//! Tests for `crate::commands::history_number`.

use super::*;

const SHOW: &str = "hianime:stitched-show-4";

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.history_path = td.path().join("history");
    state
}

/// The row a watch of the stitched recap left before it was
/// renumbered: slot 2 in the file, (2, "2.5") in the sidecar.
fn seed_former_recap_row(state: &AppState) {
    let row = crate::history::parse(&format!("2\t{SHOW}\tStitched Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(state, SHOW, 0, 2, "2.5");
}

fn repair(state: &AppState) {
    crate::commands::numbering_offset::retag_display(
        state,
        SHOW,
        &[(2, "2.5".into(), "1.5".into())],
        &|_| false,
    );
}

#[test]
fn a_repaired_row_reads_as_the_episode_it_is_and_names_what_it_read_as() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: Some("2.5".into()),
        })
    );
}

#[test]
fn a_row_no_repair_touched_reads_as_it_did() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "2.5".into(),
            former_ep_no: None,
        })
    );
}

#[test]
fn a_row_on_another_episode_names_no_former_number() {
    // The stamp was repaired, but the row has moved on to episode 5:
    // the former number is the recap's, not this episode's.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    let row = crate::history::parse(&format!("6\t{SHOW}\tStitched Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    let got = history_number(&state, SHOW).expect("read").expect("a row");
    assert_eq!(got.former_ep_no, None);
}

#[test]
fn a_show_with_no_row_has_no_number() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    assert_eq!(history_number(&state, SHOW).expect("read"), None);
}

#[tokio::test]
async fn the_route_answers_with_the_rows_number() {
    use tower::ServiceExt as _;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    let router = crate::api::build_api_router(std::sync::Arc::new(state));
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/history/hianime:stitched-show-4/number")
                .body(axum::body::Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .expect("body");
    let got: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        got,
        serde_json::json!({ "ep_no": "1.5", "former_ep_no": "2.5" })
    );
}

/// What [`history_number`] answers when `write` lands just before its
/// `n`th read of the numbering store.
fn number_with_a_write_before_read(
    n: usize,
    write: impl FnOnce(&AppState) + 'static,
) -> Option<HistoryNumber> {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    let writing = state.clone();
    crate::commands::numbering_offset::read_tap::before_read(n, move || write(&writing));
    let got = history_number(&state, SHOW).expect("read");
    crate::commands::numbering_offset::read_tap::clear();
    got
}

#[test]
fn a_number_read_while_a_repair_lands_is_the_row_before_it_or_after_it() {
    // A probe or a download repairs the stamp without the history
    // guard. Wherever the repair lands among the number read's reads,
    // the answer is the row as it was before the repair — "2.5", no
    // former number — or as it is after — "1.5", formerly "2.5" —
    // never one read of each.
    let before = Some(HistoryNumber {
        ep_no: "2.5".into(),
        former_ep_no: None,
    });
    let after = Some(HistoryNumber {
        ep_no: "1.5".into(),
        former_ep_no: Some("2.5".into()),
    });
    for n in 1..=6 {
        let got = number_with_a_write_before_read(n, repair);
        assert!(
            got == before || got == after,
            "a repair before read {n} gave {got:?}"
        );
    }
}

#[test]
fn a_list_read_while_the_offset_and_stamp_move_reads_one_of_them() {
    // The offset and the stamp move together in one write. A row read
    // across it reads both from before (episode "2.5") or both from
    // after (offset 1, slot 2 tagged "4.5": episode "3.5").
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    for n in 1..=3 {
        seed_former_recap_row(&state);
        let writing = state.clone();
        crate::commands::numbering_offset::read_tap::before_read(n, move || {
            crate::commands::numbering_offset::put_display(&writing, SHOW, 1, 2, "4.5");
        });
        let read = crate::commands::history::history_list(&state).expect("list");
        crate::commands::numbering_offset::read_tap::clear();
        let ep = read.into_iter().find(|e| e.id == SHOW).expect("row").ep_no;
        assert!(
            ep == "2.5" || ep == "3.5",
            "a write before read {n} gave {ep}"
        );
    }
}

#[test]
fn a_resume_read_while_the_offset_and_stamp_move_reads_one_of_them() {
    // The detail page's resume row, read by its Kitsu entry, reads the
    // store as the list does: both from before ("2.5") or both from
    // after ("3.5").
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    for n in 1..=3 {
        crate::history::upsert_and_write(
            &state.history_path,
            crate::history::HistoryEntry {
                ep_no: "2".into(),
                id: SHOW.into(),
                title: "Stitched Show".into(),
                watched_at: None,
                kitsu_id: Some("77".into()),
                own_number: false,
            },
        )
        .expect("row");
        crate::commands::numbering_offset::put_display(&state, SHOW, 0, 2, "2.5");
        let writing = state.clone();
        crate::commands::numbering_offset::read_tap::before_read(n, move || {
            crate::commands::numbering_offset::put_display(&writing, SHOW, 1, 2, "4.5");
        });
        let read = crate::commands::history::history_by_kitsu(&state, "77").expect("read");
        crate::commands::numbering_offset::read_tap::clear();
        let ep = read.expect("the row").ep_no;
        assert!(
            ep == "2.5" || ep == "3.5",
            "a write before read {n} gave {ep}"
        );
    }
}

#[test]
fn an_own_numbered_row_on_a_repaired_stamps_episode_names_the_former_number() {
    // A row this version wrote records the recap as "1.5" itself. Where
    // the show's stamp, from a row an earlier version wrote, was put
    // right from "2.5" to "1.5", the row is the episode that stamp
    // names, so the number it went by then still names it: a position
    // an earlier version kept under "2.5" is the same episode's.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1.5".into(),
            id: SHOW.into(),
            title: "Stitched Show".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        },
    )
    .expect("row");
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: Some("2.5".into()),
        })
    );
}

/// A listing of the stitched show with a real episode "2.5" after
/// episode 2 — a row the listing the repair read did not carry.
fn listing_with_a_real_two_point_five() -> Vec<crate::scraper::provider::EpisodeRef> {
    let ep = |n: u32, tag: Option<&str>| crate::scraper::provider::EpisodeRef {
        id: u64::from(n),
        number: n,
        number2: tag.map(str::to_string),
        former_tag: None,
        title: None,
    };
    vec![
        ep(1, None),
        ep(2, Some("1.5")),
        ep(3, Some("2")),
        ep(4, Some("2.5")),
        ep(5, Some("3")),
    ]
}

fn own_row(state: &AppState, ep_no: &str) {
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: ep_no.into(),
            id: SHOW.into(),
            title: "Stitched Show".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        },
    )
    .expect("row");
}

#[test]
fn a_former_number_a_later_listing_gives_a_real_episode_is_not_named() {
    // The recap went by "2.5" before the repair put it right to "1.5".
    // A probe since fetched a listing with a real episode "2.5", and the
    // viewer has a position kept for it. Named as the recap's former
    // number, "2.5" would have that position moved under "1.5", or
    // dropped where "1.5" has one of its own.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    own_row(&state, "1.5");
    crate::commands::numbering_offset::retag_from_listing(
        &state,
        SHOW,
        &listing_with_a_real_two_point_five(),
        None,
        None,
    );
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: None,
        })
    );
}

#[test]
fn a_legacy_row_s_former_number_a_later_listing_gives_a_real_episode_is_not_named() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    crate::commands::numbering_offset::retag_from_listing(
        &state,
        SHOW,
        &listing_with_a_real_two_point_five(),
        None,
        None,
    );
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: None,
        })
    );
}

#[test]
fn a_former_number_the_last_recorded_listing_gives_a_real_episode_is_not_named() {
    // A walk recorded the show's listing, with a real "2.5", and then
    // stopped before any stamp repair read it: its episode did not
    // serve, or the probe's mode search found nothing. The kept former
    // tag is still in the store, but the record says "2.5" is a real
    // episode's number now, so it is not named.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    own_row(&state, "1.5");
    crate::commands::listing_tags::put(
        &state,
        SHOW,
        &["1.5".to_string(), "2.5".to_string()],
        crate::commands::listing_tags::Begun::now().picked(),
    );
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: None,
        })
    );
}

#[test]
fn a_former_number_the_last_recorded_listing_leaves_free_is_named() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    own_row(&state, "1.5");
    crate::commands::listing_tags::put(
        &state,
        SHOW,
        &["1.5".to_string()],
        crate::commands::listing_tags::Begun::now().picked(),
    );
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: Some("2.5".into()),
        })
    );
}

mod former_number_props {
    use super::former_number;
    use crate::commands::numbering_offset::Numbering;
    use proptest::prelude::*;

    fn tag() -> impl Strategy<Value = String> {
        (0u32..30, prop::bool::ANY).prop_map(|(n, half)| {
            if half {
                format!("{n}.5")
            } else {
                n.to_string()
            }
        })
    }

    proptest! {
        /// Whatever the store's stamp and former tag, the row's number
        /// and the recorded tags: with a record, the row names exactly
        /// the former number it names with none, unless the record
        /// carries it; and with no record, it names what the store
        /// alone gives.
        #[test]
        fn a_recorded_listing_withholds_exactly_the_former_numbers_it_carries(
            offset in 0u32..3,
            slot in 1u32..20,
            now in tag(),
            former in tag(),
            other_row in prop::option::of(tag()),
            recorded in prop::option::of((prop::collection::vec(tag(), 0..6), prop::bool::ANY)),
        ) {
            use crate::commands::numbering_offset::kitsu_ep_no;
            let numbering = Numbering::from_body(&format!(
                "s\t{offset}\t{slot}\t{now}\t{slot}\t{former}\n"
            ));
            // The row is the stamp's episode unless another is drawn,
            // and the record carries the former number when drawn so.
            let ep_no = other_row.unwrap_or_else(|| kitsu_ep_no(&now, offset));
            let recorded = recorded.map(|(mut tags, with_former)| {
                if with_former {
                    tags.push(kitsu_ep_no(&former, offset));
                }
                tags
            });
            let alone = former_number(&numbering, "s", &ep_no, None);
            let got = former_number(&numbering, "s", &ep_no, recorded.as_deref());
            let carried = |f: &str| recorded.as_ref().is_some_and(|t| {
                crate::commands::play_native_merge::carries_tag(t, f)
            });
            match &alone {
                Some(f) if carried(f) => prop_assert_eq!(got, None),
                _ => prop_assert_eq!(got, alone),
            }
        }
    }
}
