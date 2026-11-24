//! The decisions the show page's acceptance suites other than the
//! strip's draw, asserted on the answers recorded for them
//! (`tests/fixtures/watch-order/scenarios/`):
//! `detail-watch-order-arrival` (what arrives in the held room: the
//! strip, the franchise link alone, or nothing) and
//! `detail-watch-order-franchises` (the strip on the corpus's real
//! franchises, `tests/fixtures/franchise/served-*.json`).

use serde_json::{json, Value};

use super::scenarios::{answer, ids};

const ARRIVAL: &str = "detail-watch-order-arrival";

fn arriving(scenario: &str) -> Value {
    answer(
        ARRIVAL,
        &format!("detail route — the watch-order strip arriving > {scenario}"),
        "show 42",
    )
}

#[test]
fn a_run_of_three_arrives_as_the_strip() {
    let a = arriving("holds the strip’s room from the first frame, then gives it to the strip");
    assert_eq!(ids(&a["strip"]["before"]), ["10"]);
    assert_eq!(ids(&a["strip"]["after"]), ["50"]);
    assert!(a["link"].is_null());
}

#[test]
fn the_show_alone_with_a_film_arrives_as_the_franchise_link_alone() {
    let a = arriving("holds the link’s room for a show whose franchise link comes alone");
    assert!(a["strip"].is_null());
    assert_eq!(a["link"]["similar_hidden"], json!(["60"]));
    assert_eq!(a["link"]["stack"]["summary"]["films"], 1);
}

#[test]
fn the_show_alone_arrives_as_nothing() {
    let a = arriving("holds nothing when the show’s listing foretells nothing");
    assert!(a["strip"].is_null());
    assert!(a["link"].is_null());
}

const REAL: &str = "detail-watch-order-franchises";

/// The show page's answer for `id`, recorded for `scenario` of the strip
/// on real franchises.
fn real(scenario: &str, id: &str) -> Value {
    answer(
        REAL,
        &format!("detail route — the watch-order strip on real franchises > {scenario}"),
        &format!("show {id}"),
    )
}

/// The ids of both of a strip's columns.
fn tiles(strip: &Value) -> Vec<&str> {
    let mut out = ids(&strip["before"]);
    out.extend(ids(&strip["after"]));
    out
}

/// The title of the tile for `id`.
fn tile_title<'a>(strip: &'a Value, id: &str) -> &'a str {
    strip["before"]
        .as_array()
        .into_iter()
        .chain(strip["after"].as_array())
        .flatten()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no tile {id} in {strip}"))["title"]
        .as_str()
        .expect("a title")
}

#[test]
fn one_piece_is_never_a_side_story_of_lego_one_piece_and_leads_to_its_franchise() {
    // "leads from One Piece's page to One Piece's franchise" reads the
    // same answer, recorded under this scenario.
    let a = real(
        "never calls One Piece a side story of its spin-off LEGO ONE PIECE",
        "12",
    );
    let strip = &a["strip"];
    assert_ne!(strip["kind"], "side");
    assert_ne!(strip["caption"], json!({ "kind": "side" }));
    if !strip.is_null() {
        assert!(!tiles(strip).contains(&"50656"));
    }
    assert!(
        !strip.is_null() || !a["link"].is_null(),
        "a way to the franchise page"
    );
}

#[test]
fn lego_one_piece_is_a_side_story_of_one_piece() {
    let a = real("shows LEGO ONE PIECE as a side story of One Piece", "50656");
    assert_eq!(a["strip"]["caption"], json!({ "kind": "side" }));
    assert_eq!(tile_title(&a["strip"], "12"), "One Piece");
}

#[test]
fn a_film_that_names_a_series_its_parent_reads_as_a_film_under_it() {
    for (id, parent) in [
        ("1515", "244"),
        ("45513", "12"),
        ("46874", "45619"),
        ("696", "210"),
        ("7543", "1555"),
        ("14084", "13881"),
        ("42205", "13881"),
        ("47844", "47232"),
    ] {
        let a = real(
            &format!("calls the film {id} a film, under its parent story {parent}"),
            id,
        );
        assert_eq!(a["strip"]["caption"], json!({ "kind": "film" }), "{id}");
        assert!(tiles(&a["strip"]).contains(&parent), "{id} under {parent}");
    }
}

#[test]
fn my_hero_academias_films_are_not_lined_up_as_a_story_of_their_own() {
    let a = real(
        "calls the film 14084 a film, under its parent story 13881",
        "14084",
    );
    let t = tiles(&a["strip"]);
    assert!(!t.contains(&"42205"));
    assert!(!t.contains(&"44209"));
}

#[test]
fn side_stories_that_are_not_films_read_as_side_stories() {
    for (id, what) in [
        ("428", "the 1998 One Piece OVA"),
        ("50409", "CHOPPER's TV series"),
        ("50656", "LEGO ONE PIECE"),
        ("6928", "the Fate/Zero special"),
    ] {
        // LEGO ONE PIECE's read is recorded under the scenario that
        // shows it under One Piece, with the same inputs.
        let scenario = if id == "50656" {
            "shows LEGO ONE PIECE as a side story of One Piece".to_string()
        } else {
            format!("calls {what} ({id}) a side story, not a film")
        };
        let a = real(&scenario, id);
        assert_eq!(a["strip"]["caption"], json!({ "kind": "side" }), "{id}");
    }
}

#[test]
fn the_next_entry_is_named_by_its_whole_title() {
    for (scenario, id, next, title) in [
        (
            "names Naruto's next entry by its whole title",
            "11",
            "1555",
            "Naruto: Shippuuden",
        ),
        (
            "names Bleach's next entry by its whole title, prefix and all",
            "244",
            "43078",
            "BLEACH: Sennen Kessen-hen",
        ),
        (
            "names Dragon Ball's next entry by its whole title",
            "199",
            "720",
            "Dragon Ball Z",
        ),
    ] {
        let a = real(scenario, id);
        assert!(ids(&a["strip"]["after"]).contains(&next), "{id}");
        assert_eq!(tile_title(&a["strip"], next), title);
    }
}

#[test]
fn berserks_first_golden_age_film_is_numbered_among_the_films_outside_the_main_story() {
    let a = real(
        "numbers Berserk's first Golden Age film among the films, outside the main story",
        "6084",
    );
    let s = &a["strip"];
    assert_eq!(s["outside"], true);
    assert_eq!(
        s["caption"],
        json!({ "kind": "position", "position": 1, "total": 3 })
    );
    assert!(tiles(s).contains(&"6664"));
    assert!(!tiles(s).contains(&"24"));
}

#[test]
fn the_2000_jojo_ova_comes_from_the_main_story_as_linked_and_leads_into_it_as_released() {
    let a = real(
        "shows the JoJo series the 2000 OVA follows, keyed as the main story it comes from",
        "7730",
    );
    let s = &a["strip"];
    assert_eq!(
        s["caption"],
        json!({ "kind": "position", "position": 1, "total": 2 })
    );
    assert!(tiles(s).contains(&"7158"));
    assert!(tiles(s).contains(&"610"));
    assert_eq!(s["seams"]["before"], "from");
    let r = real(
        "in release order, shows the JoJo series after the 2000 OVA, keyed as the main story it leads into",
        "7730",
    );
    assert_eq!(r["order"], "released");
    assert_eq!(
        r["strip"]["caption"],
        json!({ "kind": "position", "position": 2, "total": 2 })
    );
    assert_eq!(r["strip"]["seams"]["after"], "into");
}

#[test]
fn boruto_the_movie_is_a_film_after_shippuden_from_the_main_story() {
    let a = real(
        "calls Boruto: Naruto the Movie a film, after Shippuden from the main story",
        "10089",
    );
    let s = &a["strip"];
    assert_eq!(s["outside"], true);
    assert_eq!(s["caption"], json!({ "kind": "film" }));
    assert!(tiles(s).contains(&"1555"));
    assert_eq!(s["seams"]["before"], "from");
}

#[test]
fn a_main_story_page_says_nothing_of_a_chain() {
    let a = real("says nothing of a chain on the main story's pages", "11655");
    assert_eq!(a["strip"]["outside"], false);
    assert_ne!(a["strip"]["seams"]["before"], "from");
}

/// The ids a show's poster stack fans, front first.
fn stack_ids(a: &Value) -> Vec<&str> {
    let stack = if a["strip"].is_null() {
        &a["link"]["stack"]
    } else {
        &a["strip"]["stack"]
    };
    ids(&stack["summary"]["posters"])
}

#[test]
fn lupins_stack_leads_with_its_1971_series_not_the_1969_pilot_film() {
    let a = answer(
        REAL,
        "detail route — the franchise poster stack on real franchises > leads Lupin III's stack with its 1971 series, not the 1969 pilot film",
        "show 1265",
    );
    let s = stack_ids(&a);
    assert_eq!(s[0], "1265");
    assert!(!s.contains(&"3997"));
}

#[test]
fn every_corpus_anchors_stack_leads_with_the_entry_its_franchise_is_named_after() {
    let prefix = "detail route — the franchise poster stack on real franchises > leads ";
    let all = super::scenarios::cases(REAL);
    let mut seen = 0;
    for (name, a) in &all {
        let Some(rest) = name.strip_prefix(prefix) else {
            continue;
        };
        let Some((id, _)) = rest.split_once("'s stack with the entry") else {
            continue;
        };
        let raw = std::fs::read(
            super::fixtures::dir()
                .parent()
                .expect("fixtures")
                .join(format!("franchise/served-{id}.json")),
        )
        .expect("served file");
        let served: Value = serde_json::from_slice(&raw).expect("served json");
        assert_eq!(stack_ids(a)[0], served["name_from"], "{id}");
        seen += 1;
    }
    assert!(seen > 40, "every corpus anchor, not {seen}");
}
