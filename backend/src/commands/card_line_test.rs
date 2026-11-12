//! The line a Continue card stands for, and whether it can be corrected.

use super::*;
use proptest::prelude::*;

const CARD: &str = "hianime:jojos-bizarre-adventure-2000-5182";
const PLAYED: &str = "hianime:jojos-bizarre-adventure-2012-708";
const SERIES_2012: &str = "7158";
const OVA_2000: &str = "4013";

fn row(id: &str, from: Option<&str>, show: Option<&str>, guess: bool) -> CardRow {
    CardRow {
        id: id.into(),
        guessed_from: from.map(str::to_string),
        show: show.map(str::to_string),
        guess,
        named: false,
    }
}

fn line_of(rows: &[CardRow], id: &str) -> String {
    card_lines(rows)[id].line.clone()
}

#[test]
fn the_jojo_card_stands_for_the_2000_line_its_guessed_play_came_from() {
    let rows = [
        row(PLAYED, Some(CARD), Some(SERIES_2012), true),
        row(CARD, None, Some(SERIES_2012), true),
    ];
    assert_eq!(line_of(&rows, PLAYED), CARD);
}

#[test]
fn once_the_2000_line_is_linked_to_its_show_the_2012_row_is_its_own_card() {
    let rows = [
        row(PLAYED, Some(CARD), Some(SERIES_2012), true),
        row(CARD, None, Some(OVA_2000), false),
    ];
    assert_eq!(line_of(&rows, PLAYED), PLAYED);
    assert_eq!(line_of(&rows, CARD), CARD);
}

#[test]
fn once_the_2000_line_records_the_2012_series_the_card_stays_that_line() {
    // The user's Yes on the player: the 2012 row is still a guess, and
    // the line answered for records the same show.
    let rows = [
        row(PLAYED, Some(CARD), Some(SERIES_2012), true),
        row(CARD, None, Some(SERIES_2012), false),
    ];
    assert_eq!(line_of(&rows, PLAYED), CARD);
}

#[test]
fn the_row_stands_for_itself_when_its_line_is_gone_or_not_matched() {
    let played = row(PLAYED, Some(CARD), Some(SERIES_2012), true);
    assert_eq!(line_of(std::slice::from_ref(&played), PLAYED), PLAYED);
    let unmatched = [played.clone(), row(CARD, None, None, true)];
    assert_eq!(line_of(&unmatched, PLAYED), PLAYED);
    let shown_unmatched = [
        row(PLAYED, Some(CARD), None, true),
        row(CARD, None, None, true),
    ];
    assert_eq!(line_of(&shown_unmatched, PLAYED), PLAYED);
}

#[test]
fn a_shown_row_that_records_its_own_show_stands_for_itself() {
    // The 2012 row was played from the 2012 series' page before.
    let rows = [
        row(PLAYED, Some(CARD), Some(SERIES_2012), false),
        row(CARD, None, Some(SERIES_2012), true),
    ];
    assert_eq!(line_of(&rows, PLAYED), PLAYED);
}

#[test]
fn a_row_no_guessed_play_wrote_stands_for_itself() {
    let rows = [row(CARD, None, Some(SERIES_2012), true)];
    assert_eq!(line_of(&rows, CARD), CARD);
    let own = [row(CARD, Some(CARD), Some(SERIES_2012), true)];
    assert_eq!(line_of(&own, CARD), CARD);
}

#[test]
fn a_line_is_correctable_while_a_guess_or_once_named() {
    let named = CardRow {
        named: true,
        ..row("named-1", None, Some("1"), false)
    };
    let rows = [
        row("guess-1", None, Some("1"), true),
        named,
        row("own-1", None, Some("1"), false),
    ];
    let lines = card_lines(&rows);
    assert!(lines["guess-1"].correctable);
    assert!(lines["named-1"].correctable);
    assert!(!lines["own-1"].correctable);
}

#[test]
fn correctable_is_asked_of_the_line_the_card_stands_for() {
    // The shown 2012 row is a guess; the 2000 line it stands for is one
    // the user named for the 2012 series (the slip's Yes): the card can
    // change it again.
    let rows = [
        row(PLAYED, Some(CARD), Some(SERIES_2012), true),
        CardRow {
            named: true,
            ..row(CARD, None, Some(SERIES_2012), false)
        },
    ];
    let lines = card_lines(&rows);
    assert_eq!(
        lines[PLAYED],
        CardLine {
            line: CARD.into(),
            correctable: true
        }
    );
}

#[test]
fn the_rows_arrive_as_the_renderer_sends_them() {
    let rows: Vec<CardRow> = serde_json::from_value(serde_json::json!([
        { "id": PLAYED, "guessed_from": CARD, "show": SERIES_2012, "guess": true },
        { "id": CARD, "show": SERIES_2012, "guess": true, "named": false },
        { "id": "bare-1" }
    ]))
    .unwrap();
    assert_eq!(rows[2], row("bare-1", None, None, false));
    let wire = serde_json::to_value(card_lines(&rows)).unwrap();
    assert_eq!(
        wire[PLAYED],
        serde_json::json!({ "line": CARD, "correctable": true })
    );
}

fn world() -> impl Strategy<Value = (Vec<CardRow>, usize)> {
    let id = || proptest::sample::select(&["a", "b", "c", "d"][..]);
    proptest::collection::vec(
        (
            id(),
            proptest::option::of(id()),
            proptest::option::of(proptest::sample::select(&["k1", "k2"][..])),
            any::<bool>(),
            any::<bool>(),
        ),
        1..5,
    )
    .prop_map(|raw| {
        let mut seen = std::collections::HashSet::new();
        raw.into_iter()
            .filter(|(id, ..)| seen.insert(*id))
            .map(|(id, from, show, guess, named)| CardRow {
                id: id.into(),
                guessed_from: from.map(str::to_string),
                show: show.map(str::to_string),
                guess,
                named,
            })
            .collect::<Vec<_>>()
    })
    .prop_flat_map(|rows| {
        let n = rows.len();
        (Just(rows), 0..n)
    })
}

proptest! {
    #[test]
    fn a_card_stands_for_its_line_exactly_when_both_show_as_the_same_show(
        (rows, at) in world(),
    ) {
        let shown = &rows[at];
        let line = card_lines(&rows)[&shown.id].line.clone();
        let from = shown.guessed_from.as_deref();
        let same = shown.guess
            && from.is_some_and(|f| f != shown.id)
            && shown.show.is_some()
            && rows.iter().any(|r| Some(r.id.as_str()) == from && r.show == shown.show);
        if same {
            prop_assert_eq!(Some(line.as_str()), from);
        } else {
            prop_assert_eq!(&line, &shown.id);
        }
    }

    #[test]
    fn a_card_is_correctable_exactly_when_its_line_is_a_guess_or_named(
        (rows, at) in world(),
    ) {
        let lines = card_lines(&rows);
        let shown = &lines[&rows[at].id];
        let line = rows.iter().find(|r| r.id == shown.line).expect("a held row");
        prop_assert_eq!(shown.correctable, line.guess || line.named);
        prop_assert_eq!(lines.len(), rows.len());
    }
}
