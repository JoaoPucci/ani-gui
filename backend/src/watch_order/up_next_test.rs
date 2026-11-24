//! The player's Up next card at a finale (ported from the renderer's
//! `up-next.test.ts`). The renderer's checks that a reading of another
//! show is ignored have no counterpart: the backend reads the player's
//! own reading for the id it answers, so it can belong to no other show.

use proptest::prelude::*;

use super::chain_end::{chain_end_card, player_up_next, ChainEndCard, Finale, PlayerUpNext};
use super::fixtures::{payload, run};
use super::run::run_of;
use super::strategies::franchise_arb;
use super::up_next::{finale_card, is_finale, up_next_for, FinaleCard, FinaleSelf, UpNext};
use super::{Chain, EntryProgress, Franchise, HistoryRow, WatchEntry, WatchOrder};

use WatchOrder::{Linked, Released};

const TODAY: &str = "2026-11-02";

fn named(card: Option<&FinaleCard>) -> Option<Vec<String>> {
    card.map(|c| {
        c.up_next
            .entries()
            .into_iter()
            .map(|e| e.id.clone())
            .collect()
    })
}

fn card(
    key: &str,
    id: &str,
    order: WatchOrder,
    episode: f64,
    own: Option<&FinaleSelf>,
) -> Option<FinaleCard> {
    finale_card(&payload(key, id), &[], id, order, TODAY, episode, own)
}

fn finished(episodes: Option<u32>) -> FinaleSelf {
    FinaleSelf {
        status: Some("finished".into()),
        episodes,
    }
}

#[test]
fn up_next_for_answers_the_end_card_in_each_of_its_four_cases() {
    assert!(
        matches!(up_next_for(&run("bleach"), "43078", Linked, TODAY), UpNext::One { entry } if entry.id == "46903")
    );
    assert!(
        matches!(up_next_for(&run("monogatari"), "48434", Linked, TODAY), UpNext::Unaired { entry } if entry.id == "50013")
    );
    assert!(matches!(
        up_next_for(&run("fate"), "7658", Linked, TODAY),
        UpNext::Choices { .. }
    ));
    assert_eq!(
        up_next_for(&run("bleach"), "49444", Linked, TODAY),
        UpNext::None
    );
}

#[test]
fn is_finale_is_the_last_episode_of_an_entry_that_has_finished_airing() {
    assert!(is_finale(&finished(Some(12)), 12.0));
    assert!(!is_finale(&finished(Some(12)), 11.0));
    assert!(is_finale(&finished(Some(1)), 1.0));
    // An episode past the listed count, or an extra after the last.
    assert!(is_finale(&finished(Some(12)), 13.0));
    assert!(is_finale(&finished(Some(12)), 12.5));
    assert!(!is_finale(&finished(Some(12)), 11.5));
}

#[test]
fn is_finale_has_no_finale_while_the_entry_airs_or_with_no_episode_count() {
    for status in [
        Some("current"),
        Some("upcoming"),
        Some("tba"),
        Some("unreleased"),
        None,
    ] {
        let own = FinaleSelf {
            status: status.map(Into::into),
            episodes: Some(12),
        };
        assert!(!is_finale(&own, 12.0));
    }
    assert!(!is_finale(&finished(None), 12.0));
}

#[test]
fn finale_card_names_the_next_entry_at_a_finale() {
    let got = card("bleach", "43078", Linked, 13.0, None).expect("a card");
    assert!(matches!(got.up_next, UpNext::One { entry } if entry.id == "46903"));
}

#[test]
fn finale_card_shows_no_card_before_the_finale_or_after_the_last_entry() {
    assert_eq!(card("bleach", "43078", Linked, 12.0, None), None);
    assert_eq!(card("bleach", "49444", Linked, 10.0, None), None);
}

#[test]
fn finale_card_ignores_a_franchise_read_for_another_id() {
    assert_eq!(
        finale_card(
            &payload("bleach", "244"),
            &[],
            "43078",
            Linked,
            TODAY,
            13.0,
            None
        ),
        None
    );
}

#[test]
fn finale_card_shows_no_card_for_a_franchise_read_only_in_part() {
    let mut part = payload("bleach", "43078");
    part.complete = false;
    for order in [Linked, Released] {
        assert_eq!(
            finale_card(&part, &[], "43078", order, TODAY, 13.0, None),
            None
        );
    }
}

#[test]
fn finale_card_shows_no_card_for_a_side_story_outside_the_run() {
    assert_eq!(card("fate", "6928", Linked, 1.0, None), None);
    assert_eq!(card("fate", "6928", Released, 1.0, None), None);
}

#[test]
fn finale_card_follows_the_order_setting() {
    // Fate/Zero 2nd Season forks in link order; released, the prologue alone follows.
    let linked = card("fate", "7658", Linked, 12.0, None);
    assert!(matches!(
        linked.as_ref().map(|c| &c.up_next),
        Some(UpNext::Choices { .. })
    ));
    assert_eq!(
        named(linked.as_ref()),
        Some(vec!["323".into(), "9718".into()])
    );
    let released = card("fate", "7658", Released, 12.0, None).expect("a card");
    assert!(matches!(released.up_next, UpNext::One { entry } if entry.id == "9718"));
    // Fate/stay night (2006) ends its branch when linked, and leads to Fate/Zero when released.
    assert_eq!(card("fate", "323", Linked, 24.0, None), None);
    assert_eq!(
        named(card("fate", "323", Released, 24.0, None).as_ref()),
        Some(vec!["6028".into()])
    );
}

#[test]
fn finale_card_names_a_next_entry_that_has_not_aired_as_information() {
    let got = card("monogatari", "48434", Linked, 14.0, None).expect("a card");
    assert!(matches!(got.up_next, UpNext::Unaired { entry } if entry.id == "50013"));
}

#[test]
fn finale_card_decides_the_finale_by_the_players_own_reading_when_it_has_one() {
    // The run lists Stone Ocean Part 3 at 14 episodes, finished.
    assert!(card("jojo", "46598", Linked, 14.0, None).is_some());
    assert!(card("jojo", "46598", Linked, 12.0, None).is_none());
    // The player's reading wins where the two differ.
    assert!(card("jojo", "46598", Linked, 12.0, Some(&finished(Some(12)))).is_some());
    let airing = FinaleSelf {
        status: Some("current".into()),
        episodes: Some(14),
    };
    assert!(card("jojo", "46598", Linked, 14.0, Some(&airing)).is_none());
}

#[test]
fn finale_card_carries_the_next_entries_progress_from_the_history() {
    let rows = [
        HistoryRow {
            kitsu_id: Some("46903".into()),
            ep_no: "4".into(),
        },
        HistoryRow {
            kitsu_id: Some("43078".into()),
            ep_no: "13".into(),
        },
    ];
    let got = finale_card(
        &payload("bleach", "43078"),
        &rows,
        "43078",
        Linked,
        TODAY,
        13.0,
        None,
    )
    .expect("a card");
    assert_eq!(
        got.progress.get("46903"),
        Some(&EntryProgress::Episode(4.0))
    );
}

fn e(id: &str, start: &str, subtype: &str) -> WatchEntry {
    WatchEntry {
        id: id.into(),
        title: format!("T{id}"),
        subtype: Some(subtype.into()),
        start: Some(start.into()),
        episodes: Some(1),
        status: Some("finished".into()),
        poster: None,
        cover: None,
    }
}

fn l(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

/// A main story of two, and a chain of two films that leads into its
/// second title (`into`) or nowhere.
fn chain_data(anchor: &str, into: bool) -> Franchise {
    let main = vec![e("1", "2000-01-01", "TV"), e("2", "2004-01-01", "TV")];
    let (f1, f2) = (
        e("10", "2001-01-01", "movie"),
        e("11", "2002-01-01", "movie"),
    );
    let chain = if into {
        Chain {
            entries: vec![f1.clone(), f2.clone(), main[1].clone()],
            links: vec![l("10", "11"), l("11", "2")],
            seams: vec!["2".into()],
        }
    } else {
        Chain {
            entries: vec![f1.clone(), f2.clone()],
            links: vec![l("10", "11")],
            seams: Vec::new(),
        }
    };
    Franchise {
        anchor: anchor.into(),
        name: "The franchise".into(),
        name_from: "1".into(),
        entries: main,
        links: vec![l("1", "2")],
        versions: Vec::new(),
        films: vec![f1, f2],
        sides: Vec::new(),
        recaps: Vec::new(),
        parents: Default::default(),
        complete: true,
        chain: Some(chain),
    }
}

fn at<'a>(id: &'a str, order: WatchOrder, own: Option<&'a FinaleSelf>) -> Finale<'a> {
    Finale {
        id,
        order,
        today: TODAY,
        episode: 1.0,
        own,
    }
}

fn end(f: &Franchise, own: Option<&FinaleSelf>) -> Option<ChainEndCard> {
    chain_end_card(f, &[], at(&f.anchor, Linked, own))
}

#[test]
fn outside_the_main_story_names_the_chains_next_title() {
    let f = chain_data("10", false);
    assert_eq!(
        named(finale_card(&f, &[], "10", Linked, TODAY, 1.0, None).as_ref()),
        Some(vec!["11".into()])
    );
}

#[test]
fn goes_on_into_the_main_story_at_the_end_of_a_chain_that_leads_into_it() {
    let f = chain_data("11", true);
    assert_eq!(
        named(finale_card(&f, &[], "11", Linked, TODAY, 1.0, None).as_ref()),
        Some(vec!["2".into()])
    );
}

#[test]
fn names_nothing_at_the_end_of_a_chain_that_leads_nowhere_and_offers_the_franchise_page() {
    let f = chain_data("11", false);
    assert_eq!(finale_card(&f, &[], "11", Linked, TODAY, 1.0, None), None);
    let got = end(&f, None).expect("a franchise card");
    assert_eq!(got.name, "The franchise");
    assert_eq!(got.id, "11");
    // The strip link's stack: the main story's two titles, the films.
    assert_eq!((got.summary.main, got.summary.films), (2, 2));
    assert_eq!(got.front, "1");
}

#[test]
fn offers_no_franchise_card_where_the_card_names_a_next_title_or_short_of_the_finale() {
    assert_eq!(end(&chain_data("10", false), None), None);
    assert_eq!(end(&chain_data("11", true), None), None);
    let unfinished = FinaleSelf {
        status: Some("current".into()),
        episodes: Some(1),
    };
    assert_eq!(end(&chain_data("11", false), Some(&unfinished)), None);
}

#[test]
fn offers_no_franchise_card_in_a_franchise_read_in_part_or_on_a_main_story_page() {
    let mut cut = chain_data("11", false);
    cut.complete = false;
    assert_eq!(end(&cut, None), None);
    let main = Franchise {
        anchor: "2".into(),
        chain: None,
        ..chain_data("11", false)
    };
    assert_eq!(end(&main, None), None);
}

#[test]
fn fronts_the_franchise_card_as_its_page_does_when_the_name_comes_from_no_main_story_title() {
    let off = Franchise {
        name_from: "99".into(),
        ..chain_data("11", false)
    };
    // The franchise page takes the main story's first title then.
    assert_eq!(end(&off, None).expect("a franchise card").front, "1");
}

#[test]
fn shows_one_card_the_next_title_where_the_finale_names_one_else_the_franchise() {
    let next = player_up_next(&chain_data("10", false), &[], at("10", Linked, None));
    assert!(matches!(next, Some(PlayerUpNext::Entry(_))));
    let f = chain_data("11", false);
    let Some(PlayerUpNext::Franchise(card)) = player_up_next(&f, &[], at("11", Linked, None))
    else {
        panic!("the franchise card");
    };
    assert_eq!(card.name, "The franchise");
    let unfinished = FinaleSelf {
        status: Some("current".into()),
        episodes: Some(1),
    };
    assert_eq!(
        player_up_next(&f, &[], at("11", Linked, Some(&unfinished))),
        None
    );
}

fn order() -> BoxedStrategy<WatchOrder> {
    prop::sample::select(&[Linked, Released][..]).boxed()
}

proptest! {
    #[test]
    fn is_finale_holds_from_the_last_episode_on_and_only_for_a_finished_entry_with_a_count(
        status in prop::option::of(prop::sample::select(&["finished", "current", "tba"][..])),
        episodes in prop::option::of(1u32..=400),
        episode in 0.0f64..500.0,
        later in 0.0f64..50.0,
    ) {
        let own = FinaleSelf { status: status.map(Into::into), episodes };
        let got = is_finale(&own, episode);
        prop_assert_eq!(got, status == Some("finished") && episodes.is_some_and(|n| episode >= f64::from(n)));
        if got {
            prop_assert!(is_finale(&own, episode + later));
        }
    }

    #[test]
    fn finale_card_names_only_run_members_after_the_entry_as_the_rule_does(
        franchise in franchise_arb(),
        order in order(),
        episode in 1u32..=40,
        reading in prop::option::of((
            prop::option::of(prop::sample::select(&["finished", "current"][..])),
            prop::option::of(1u32..=40),
        )),
    ) {
        let id = franchise.anchor.clone();
        let own = reading.map(|(status, episodes)| FinaleSelf { status: status.map(Into::into), episodes });
        let episode = f64::from(episode);
        let Some(got) = finale_card(&franchise, &[], &id, order, TODAY, episode, own.as_ref()) else { return Ok(()) };
        let r = run_of(&franchise).expect("a run");
        let entry = r.entries.iter().find(|e| e.id == id).expect("a member");
        let decides = own.unwrap_or_else(|| entry.into());
        prop_assert!(is_finale(&decides, episode));
        for n in got.up_next.entries() {
            prop_assert!(r.entries.iter().any(|e| e.id == n.id));
            prop_assert_ne!(&n.id, &id);
        }
        prop_assert_eq!(got.up_next, up_next_for(&r, &id, order, TODAY));
    }

    #[test]
    fn shows_a_card_wherever_the_rule_names_something_at_a_finished_finale_read_whole(
        franchise in franchise_arb(),
        order in order(),
    ) {
        let id = franchise.anchor.clone();
        let Some(r) = run_of(&franchise) else { return Ok(()) };
        if !r.entries.iter().any(|e| e.id == id) {
            return Ok(());
        }
        let own = finished(Some(3));
        let got = finale_card(&franchise, &[], &id, order, TODAY, 3.0, Some(&own));
        let rule = up_next_for(&r, &id, order, TODAY);
        prop_assert_eq!(got.is_none(), rule == UpNext::None || !franchise.complete);
    }

    #[test]
    fn player_up_next_is_the_finale_card_where_there_is_one_else_the_chain_end_card_else_none(
        franchise in franchise_arb(),
        order in order(),
        episode in 1u32..=40,
    ) {
        let id = franchise.anchor.clone();
        let here = Finale { id: &id, order, today: TODAY, episode: f64::from(episode), own: None };
        let finale = finale_card(&franchise, &[], &id, order, TODAY, here.episode, None);
        let end = chain_end_card(&franchise, &[], here);
        let shown = player_up_next(&franchise, &[], here);
        match (finale, end) {
            (Some(card), _) => prop_assert_eq!(shown, Some(PlayerUpNext::Entry(card))),
            (None, Some(card)) => prop_assert_eq!(shown, Some(PlayerUpNext::Franchise(card))),
            (None, None) => prop_assert_eq!(shown, None),
        }
    }
}
