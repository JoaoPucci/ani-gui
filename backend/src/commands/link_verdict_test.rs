//! Whether a surface asks about a card's line, and which rows a
//! correction of it covers.

use super::*;
use proptest::prelude::*;

const GUESS: &str = "12625";

fn on_page(show: Option<&str>) -> VerdictArgs {
    VerdictArgs {
        page: GUESS.into(),
        show: show.map(str::to_string),
        asks_guess: true,
        ..VerdictArgs::default()
    }
}

fn offer(args: &VerdictArgs) -> Option<OfferKind> {
    verdict(args).offer
}

#[test]
fn a_line_guessed_as_the_page_is_asked_about() {
    assert_eq!(offer(&on_page(Some(GUESS))), Some(OfferKind::Guess));
}

#[test]
fn a_line_guessed_as_another_show_or_matching_none_is_not() {
    assert_eq!(offer(&on_page(Some("13209"))), None);
    assert_eq!(offer(&on_page(None)), None);
}

#[test]
fn a_trusted_line_is_not_asked_about() {
    let trusted = VerdictArgs {
        trusted: true,
        ..on_page(Some(GUESS))
    };
    assert_eq!(offer(&trusted), None);
}

#[test]
fn a_surface_that_may_not_ask_about_a_guess_does_not() {
    let quiet = VerdictArgs {
        asks_guess: false,
        ..on_page(Some(GUESS))
    };
    assert_eq!(offer(&quiet), None);
}

#[test]
fn a_session_the_guessed_card_opened_asks_though_its_play_recorded_the_guess() {
    let carried = VerdictArgs {
        trusted: true,
        carried: true,
        ..on_page(Some(GUESS))
    };
    assert_eq!(offer(&carried), Some(OfferKind::Guess));
}

#[test]
fn a_carried_session_offers_a_named_line_only_again() {
    let named = VerdictArgs {
        trusted: true,
        carried: true,
        named: true,
        ..on_page(Some(GUESS))
    };
    assert_eq!(offer(&named), Some(OfferKind::Again));
}

#[test]
fn a_line_picked_on_the_player_is_only_offered_again() {
    let picked = VerdictArgs {
        trusted: true,
        carried: true,
        picked: true,
        ..on_page(Some(GUESS))
    };
    assert_eq!(offer(&picked), Some(OfferKind::Again));
}

#[test]
fn a_named_line_is_offered_again_on_every_surface_while_it_shows_the_page() {
    let named = VerdictArgs {
        trusted: true,
        named: true,
        asks_guess: false,
        ..on_page(Some(GUESS))
    };
    assert_eq!(offer(&named), Some(OfferKind::Again));
    let elsewhere = VerdictArgs {
        show: Some("4013".into()),
        ..named
    };
    assert_eq!(offer(&elsewhere), None);
}

#[test]
fn a_line_the_app_recorded_from_a_play_is_not_offered_again() {
    let recorded = VerdictArgs {
        trusted: true,
        ..on_page(Some(GUESS))
    };
    assert_eq!(
        verdict(&recorded),
        Verdict {
            offer: None,
            kept: false
        }
    );
}

#[test]
fn a_line_named_for_another_show_than_the_one_shown_is_the_pick_kept() {
    let kept = VerdictArgs {
        trusted: true,
        named: true,
        ..on_page(Some("4013"))
    };
    assert!(verdict(&kept).kept);
    let same = VerdictArgs {
        show: Some(GUESS.into()),
        ..kept.clone()
    };
    assert!(!verdict(&same).kept);
    let unnamed = VerdictArgs {
        named: false,
        ..kept.clone()
    };
    assert!(!verdict(&unnamed).kept);
    let unmatched = VerdictArgs { show: None, ..kept };
    assert!(!verdict(&unmatched).kept);
}

#[test]
fn on_the_player_a_kept_pick_is_offered_again() {
    // The player mounted again after "Keep watching this": the line is
    // named for the pick while the old show plays. Its ⋯ entry stays, to
    // change the show again; elsewhere the line shows another show.
    let kept = VerdictArgs {
        trusted: true,
        named: true,
        in_player: true,
        ..on_page(Some("4013"))
    };
    assert_eq!(
        verdict(&kept),
        Verdict {
            offer: Some(OfferKind::Again),
            kept: true
        }
    );
    let elsewhere = VerdictArgs {
        in_player: false,
        ..kept
    };
    assert_eq!(verdict(&elsewhere).offer, None);
}

#[test]
fn the_verdict_arrives_and_answers_as_the_renderer_speaks_it() {
    let args: VerdictArgs = serde_json::from_value(serde_json::json!({
        "page": GUESS, "show": GUESS, "trusted": false, "named": false,
        "asks_guess": true, "carried": false, "picked": false
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(verdict(&args)).unwrap(),
        serde_json::json!({ "offer": "guess", "kept": false })
    );
    let bare: VerdictArgs = serde_json::from_value(serde_json::json!({ "page": GUESS })).unwrap();
    assert_eq!(
        serde_json::to_value(verdict(&bare)).unwrap(),
        serde_json::json!({ "offer": null, "kept": false })
    );
}

const CARD: &str = "hianime:black-clover-949";

fn matched(id: &str, show: Option<&str>, trusted: bool) -> RowMatch {
    RowMatch {
        id: id.into(),
        show: show.map(str::to_string),
        trusted,
    }
}

#[test]
fn a_correction_covers_the_card_and_every_row_guessed_as_the_same_show() {
    let rows = [
        matched("black-clover-77", Some(GUESS), false),
        matched("hianime:black-clover-special-3392", Some(GUESS), true),
        matched("hianime:gintama-1237", Some("818"), false),
    ];
    assert_eq!(
        group_members(CARD, GUESS, None, &rows),
        [CARD, "black-clover-77"]
    );
}

#[test]
fn a_row_whose_match_could_not_be_read_stays_out() {
    let rows = [
        matched("black-clover-77", None, false),
        matched("black-clover-78", Some(GUESS), false),
    ];
    assert_eq!(
        group_members(CARD, GUESS, None, &rows),
        [CARD, "black-clover-78"]
    );
}

#[test]
fn a_named_line_covers_the_rows_recorded_as_its_show_trusted_or_not() {
    let rows = [
        matched("black-clover-77", Some("4013"), true),
        matched("hianime:black-clover-special-3392", Some(GUESS), true),
        matched("black-clover-78", Some("4013"), false),
    ];
    assert_eq!(
        group_members(CARD, GUESS, Some("4013"), &rows),
        [CARD, "black-clover-77", "black-clover-78"]
    );
}

#[test]
fn the_card_is_named_once_and_first_whatever_the_rows_say() {
    let rows = [
        matched("black-clover-77", Some(GUESS), false),
        matched(CARD, Some(GUESS), false),
    ];
    assert_eq!(
        group_members(CARD, GUESS, None, &rows),
        [CARD, "black-clover-77"]
    );
}

#[test]
fn the_rows_arrive_as_the_renderer_sends_them() {
    let rows: Vec<RowMatch> = serde_json::from_value(serde_json::json!([
        { "id": "a", "show": GUESS, "trusted": false },
        { "id": "b", "show": null },
        { "id": "c" }
    ]))
    .unwrap();
    assert_eq!(rows[1], matched("b", None, false));
    assert_eq!(rows[2], matched("c", None, false));
}

fn flags() -> impl Strategy<Value = VerdictArgs> {
    (
        proptest::option::of(proptest::sample::select(&["1", "2"][..])),
        any::<[bool; 6]>(),
    )
        .prop_map(
            |(show, [trusted, named, asks_guess, carried, picked, in_player])| VerdictArgs {
                page: "1".into(),
                show: show.map(str::to_string),
                trusted,
                named,
                asks_guess,
                carried,
                picked,
                in_player,
            },
        )
}

proptest! {
    #[test]
    fn an_offer_is_made_only_for_a_line_showing_the_page_or_kept_on_the_player(args in flags()) {
        let shows_page = args.show.as_deref() == Some(args.page.as_str());
        let v = verdict(&args);
        if !shows_page {
            // Only the player offers a kept pick again.
            let again = args.in_player && v.kept;
            prop_assert_eq!(v.offer, again.then_some(OfferKind::Again));
        }
        prop_assert_eq!(v.kept, args.named && args.show.is_some() && !shows_page);
    }

    #[test]
    fn a_guess_is_asked_exactly_when_the_surface_may_and_the_line_is_a_guess_to_it(
        args in flags(),
    ) {
        let shows_page = args.show.as_deref() == Some(args.page.as_str());
        let guess = args.asks_guess
            && shows_page
            && (!args.trusted || (args.carried && !args.picked && !args.named));
        prop_assert_eq!(verdict(&args).offer == Some(OfferKind::Guess), guess);
        let named_here = shows_page && (args.picked || args.named);
        let kept_on_player = args.in_player && args.named && args.show.is_some() && !shows_page;
        let again = !guess && (named_here || kept_on_player);
        prop_assert_eq!(verdict(&args).offer == Some(OfferKind::Again), again);
    }

    #[test]
    fn a_correction_covers_the_card_once_first_and_only_rows_matching_its_show(
        rows in proptest::collection::vec(
            (
                proptest::sample::select(&["c", "a", "b", "d"][..]),
                proptest::option::of(proptest::sample::select(&["1", "2"][..])),
                any::<bool>(),
            ),
            0..6,
        ),
        saved in proptest::option::of(proptest::sample::select(&["1", "2"][..])),
    ) {
        // Distinct ids, as the history keeps them.
        let mut seen = std::collections::HashSet::new();
        let rows: Vec<RowMatch> = rows
            .into_iter()
            .filter(|(id, ..)| seen.insert(*id))
            .map(|(id, show, trusted)| matched(id, show, trusted))
            .collect();
        let out = group_members("c", "1", saved, &rows);
        prop_assert_eq!(out[0].as_str(), "c");
        prop_assert_eq!(out.iter().filter(|r| r.as_str() == "c").count(), 1);
        // Every row the correction covers is in it.
        for r in rows.iter().filter(|r| r.id != "c") {
            let covered = match saved {
                None => r.show.as_deref() == Some("1") && !r.trusted,
                Some(s) => r.show.as_deref() == Some(s),
            };
            prop_assert_eq!(out.contains(&r.id), covered);
        }
        for member in &out[1..] {
            let r = rows.iter().find(|r| &r.id == member).expect("asked");
            match saved {
                None => prop_assert!(r.show.as_deref() == Some("1") && !r.trusted),
                Some(s) => prop_assert_eq!(r.show.as_deref(), Some(s)),
            }
        }
    }
}
