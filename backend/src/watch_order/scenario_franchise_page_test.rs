//! The decisions `frontend/tests/acceptance/franchise-page.test.ts` and
//! `franchise-page-franchises.test.ts` draw, asserted on the answers
//! recorded for them (`tests/fixtures/watch-order/scenarios/`).
//!
//! `franchise-page`: the Saga franchise, Origins → Saga → Second Arc →
//! Final Arc by its links, Origins released between Second Arc and
//! Final Arc, with a remake, a film, a side story and a recap, read
//! for Second Arc (42). `franchise-page-franchises`: every corpus
//! franchise the walk answers (`tests/fixtures/franchise/served-*.json`),
//! in link order with nothing watched.

use serde_json::{json, Value};

use super::scenarios::{answer, ids};

const SUITE: &str = "franchise-page";
const PAGE: &str = "franchise page";
const PROGRESS: &str = "franchise page progress";

/// The page recorded for scenario `scenario` of `describe`, read for `id`.
fn page_in(describe: &str, scenario: &str, id: &str) -> Value {
    answer(
        SUITE,
        &format!("{describe} > {scenario}"),
        &format!("franchise {id}"),
    )["page"]
        .clone()
}

fn page(scenario: &str) -> Value {
    page_in(PAGE, scenario, "42")
}

/// The answer's order, beside the page.
fn order(scenario: &str) -> Value {
    answer(SUITE, &format!("{PAGE} > {scenario}"), "franchise 42")["order"].clone()
}

/// Every row of the main story, in the order the page draws them.
fn rows(p: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    for s in p["sections"].as_array().expect("sections") {
        match s["kind"].as_str().expect("a section kind") {
            "ways" => {
                for way in s["ways"].as_array().expect("ways") {
                    out.extend(way.as_array().expect("a way"));
                }
            }
            _ => out.extend(s["rows"].as_array().expect("rows")),
        }
    }
    out
}

/// The entry ids of a list of rows.
fn ids_of_rows(list: &Value) -> Vec<&str> {
    list.as_array()
        .expect("rows")
        .iter()
        .map(|r| r["entry"]["id"].as_str().expect("an id"))
        .collect()
}

fn row_ids(p: &Value) -> Vec<&str> {
    rows(p)
        .iter()
        .map(|r| r["entry"]["id"].as_str().expect("an id"))
        .collect()
}

fn row<'a>(p: &'a Value, id: &str) -> &'a Value {
    rows(p)
        .into_iter()
        .find(|r| r["entry"]["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn positions(p: &Value) -> Vec<Value> {
    rows(p).iter().map(|r| r["position"].clone()).collect()
}

/// The one section of kind `kind`.
fn section<'a>(p: &'a Value, kind: &str) -> &'a Value {
    let all: Vec<&Value> = p["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .filter(|s| s["kind"] == kind)
        .collect();
    assert_eq!(all.len(), 1, "one {kind} section in {}", p["sections"]);
    all[0]
}

/// The main story's last section.
fn last_section(p: &Value) -> &Value {
    p["sections"]
        .as_array()
        .expect("sections")
        .last()
        .expect("a section")
}

const WHOLE: &str = "shows the name the franchise read gives and counts what it lists";
const PARTIAL: &str = "says when there may be more titles than it shows";

#[test]
fn the_page_is_named_saga_after_its_first_series_and_counts_its_eight_titles() {
    let p = page(WHOLE);
    assert_eq!(p["name"], "Saga");
    // Named after Saga, not Origins, the OVA first in link order.
    assert_eq!(p["art"]["id"], "10");
    assert_eq!(p["total"], 8);
    assert_eq!(p["alone"], false);
    assert_eq!(p["partial"], false);
}

#[test]
fn link_order_numbers_origins_saga_second_arc_final_arc_with_second_arc_marked() {
    // "lists the main story in link order, numbered, with the show it
    // came from marked" reads these same inputs.
    let p = page(WHOLE);
    assert_eq!(order(WHOLE), "linked");
    assert_eq!(row_ids(&p), ["5", "10", "42", "50"]);
    assert_eq!(positions(&p), [json!(1), json!(2), json!(3), json!(4)]);
    let here: Vec<&str> = rows(&p)
        .iter()
        .filter(|r| r["here"] == true)
        .map(|r| r["entry"]["id"].as_str().expect("id"))
        .collect();
    assert_eq!(here, ["42"]);
    assert_eq!(row(&p, "50")["entry"]["title"], "Saga: Final Arc");
}

#[test]
fn release_order_puts_origins_third() {
    let s = "follows the order setting";
    let p = page(s);
    assert_eq!(order(s), "released");
    assert_eq!(row_ids(&p), ["10", "42", "5", "50"]);
    assert_eq!(row(&p, "5")["position"], 3);
}

#[test]
fn resumes_final_arc_at_episode_three_fourth_of_four_with_saga_watched() {
    let p = page("resumes the title being watched, and says where it is");
    let r = &p["resume"];
    assert_eq!(r["entry"]["id"], "50");
    assert_eq!(r["entry"]["title"], "Saga: Final Arc");
    assert_eq!(r["episode"], 3.0);
    assert_eq!(r["position"], 4);
    assert_eq!(
        row(&p, "10")["entry"]["progress"],
        json!({ "kind": "watched" })
    );
}

#[test]
fn a_row_named_for_final_arc_after_kitsu_deleted_its_entry_resumes_final_arc() {
    let p =
        page("resumes the title a row is named for when Kitsu deleted the entry the row recorded");
    assert_eq!(p["resume"]["entry"]["id"], "50");
    assert_eq!(p["resume"]["episode"], 3.0);
}

#[test]
fn a_row_linked_to_final_arc_while_the_page_is_open_resumes_it() {
    // Both link scenarios read the page again over the history the link
    // wrote; "follows a link's rows once it answers, without blanking
    // the page" sends these same inputs.
    let p = page("follows a link answered while its rows were still being read");
    assert_eq!(p["resume"]["entry"]["id"], "50");
    assert_eq!(p["resume"]["episode"], 3.0);
    // Before the link nothing was part-way: the page offered to start.
    assert!(page(WHOLE)["resume"].is_null());
}

#[test]
fn with_nothing_part_way_it_offers_to_start_from_origins() {
    // "offers to start from the first title when nothing is part-way".
    let p = page(WHOLE);
    assert!(p["resume"].is_null());
    assert_eq!(p["start"]["id"], "5");
}

#[test]
fn other_versions_films_side_stories_and_recaps_go_on_their_rails() {
    // "puts other versions, films, side stories and recaps on rails".
    let p = page(WHOLE);
    assert_eq!(ids(&p["versions"]), ["70"]);
    assert_eq!(ids(&p["films"]), ["60"]);
    assert_eq!(ids(&p["sides"]), ["80"]);
    assert_eq!(ids(&p["recaps"]), ["90"]);
}

#[test]
fn a_franchise_with_nothing_beside_its_main_story_has_no_rails_and_counts_four() {
    let p = page("leaves out the rails the franchise has nothing for");
    for rail in ["versions", "films", "sides", "recaps"] {
        assert_eq!(ids(&p[rail]), Vec::<&str>::new(), "{rail}");
    }
    assert_eq!(p["total"], 4);
}

#[test]
fn where_the_story_branches_final_arc_and_its_alternate_are_two_ways_of_one_title() {
    let p = page("sets the ways to continue side by side where the story branches");
    let ways = section(&p, "ways")["ways"].as_array().expect("ways");
    let firsts: Vec<&str> = ways
        .iter()
        .map(|w| {
            assert_eq!(w.as_array().expect("a way").len(), 1);
            w[0]["entry"]["id"].as_str().expect("id")
        })
        .collect();
    assert_eq!(firsts, ["50", "51"]);
    assert_eq!(p["partial"], false);
}

#[test]
fn a_franchise_read_in_part_says_so_and_claims_no_numbers_total_or_beginning() {
    // "says when there may be more titles than it shows" and "claims no
    // places, total or beginning when there may be more titles".
    let p = page(PARTIAL);
    assert_eq!(p["partial"], true);
    assert!(p["total"].is_null());
    assert!(p["start"].is_null());
    assert!(positions(&p).iter().all(Value::is_null));
    assert_eq!(row_ids(&p), ["5", "10", "42", "50"]);
}

#[test]
fn a_franchise_read_in_part_still_resumes_final_arc_with_no_place() {
    let p =
        page("still resumes the title being watched when there may be more titles, with no place");
    assert_eq!(p["resume"]["entry"]["id"], "50");
    assert!(p["resume"]["position"].is_null());
    assert!(positions(&p).iter().all(Value::is_null));
}

#[test]
fn ways_on_from_a_franchise_read_in_part_carry_no_count() {
    let p = page("heads ways on without a count when there may be more titles");
    assert_eq!(
        section(&p, "ways")["ways"].as_array().expect("ways").len(),
        2
    );
    assert_eq!(p["partial"], true);
    assert!(p["total"].is_null());
}

#[test]
fn a_loop_read_whole_is_one_group_of_its_three_titles() {
    let p = page("counts the titles of a loop when the franchise was read whole");
    assert_eq!(
        ids_of_rows(&section(&p, "group")["rows"]),
        ["10", "42", "50"]
    );
    assert_eq!(p["partial"], false);
    assert!(positions(&p).iter().all(Value::is_null));
}

#[test]
fn a_loop_read_in_part_is_a_group_with_no_count() {
    let p = page("heads a loop without a count when there may be more titles");
    assert_eq!(
        ids_of_rows(&section(&p, "group")["rows"]),
        ["10", "42", "50"]
    );
    assert_eq!(p["partial"], true);
}

#[test]
fn the_recaps_are_counted_only_for_a_franchise_read_whole() {
    // "counts the recaps only when the franchise was read whole" and
    // "gives the recaps no count when there may be more titles".
    assert_eq!(page(WHOLE)["recap_count"], 1);
    let partial = page(PARTIAL);
    assert!(partial["recap_count"].is_null());
    assert_eq!(ids(&partial["recaps"]), ["90"]);
}

#[test]
fn the_trail_names_second_arc_the_show_it_was_opened_from() {
    // "names the show it was opened from in the topbar".
    assert_eq!(
        page(WHOLE)["opened_from"],
        json!({ "id": "42", "title": "Saga: Second Arc" })
    );
}

#[test]
fn read_for_final_arc_the_page_marks_final_arc_and_is_opened_from_it() {
    // "shows the franchise of the show it moves to, not the one before"
    // reads these same inputs.
    let p = page_in(
        PAGE,
        "keeps the latest read when an older one for the same show answers after it",
        "50",
    );
    assert_eq!(row(&p, "50")["here"], true);
    assert_eq!(row(&p, "42")["here"], false);
    assert_eq!(p["opened_from"]["id"], "50");
}

#[test]
fn a_franchise_that_is_the_show_alone_is_alone() {
    let p = page("says the show has nothing around it when the franchise is the show alone");
    assert_eq!(p["alone"], true);
}

#[test]
fn a_franchise_with_no_main_story_has_no_page() {
    let a = answer(
        SUITE,
        &format!(
            "{PAGE} > does not claim the show is alone when the answer lists titles around it"
        ),
        "franchise 42",
    );
    assert!(a["page"].is_null());
}

#[test]
fn titles_with_no_clear_place_close_the_main_story_after_final_arc_third() {
    let p = page("closes the main story with the titles that have no clear place");
    let last = last_section(&p);
    assert_eq!(last["kind"], "line");
    assert_eq!(last["unplaced"], true);
    assert_eq!(ids_of_rows(&last["rows"]), ["5", "45"]);
    assert!(last["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .all(|r| r["position"].is_null()));
    assert_eq!(row(&p, "50")["position"], 3);
    assert_eq!(row(&p, "5")["leads_into"], "Saga: Side Arc");
}

#[test]
fn titles_with_no_clear_place_stay_apart_in_a_franchise_read_in_part() {
    let p = page("keeps the titles with no clear place apart when there may be more titles");
    let last = last_section(&p);
    assert_eq!(last["unplaced"], true);
    assert_eq!(ids_of_rows(&last["rows"]), ["5", "45"]);
}

#[test]
fn a_franchise_whose_one_title_is_another_show_is_not_alone() {
    let p = page("draws the page when the one title it lists is another show");
    assert_eq!(p["alone"], false);
    assert_eq!(row_ids(&p), ["50"]);
    assert!(p["opened_from"].is_null());
}

#[test]
fn each_row_carries_how_far_it_is_watched() {
    // Both locales read the same inputs.
    let p = page_in(
        PROGRESS,
        "captions each row by how far it is watched, in en",
        "42",
    );
    let partway = row(&p, "42");
    assert_eq!(
        partway["entry"]["progress"],
        json!({ "kind": "partway", "episode": 6.0, "total": 12 })
    );
    assert_eq!(partway["share"], 0.5);
    assert_eq!(
        row(&p, "10")["entry"]["progress"],
        json!({ "kind": "watched" })
    );
    // Still airing with no known total: its episode alone, no line.
    let airing = row(&p, "50");
    assert_eq!(
        airing["entry"]["progress"],
        json!({ "kind": "partway", "episode": 5.0, "total": null })
    );
    assert_eq!(airing["share"], 0.0);
    assert_eq!(row(&p, "5")["entry"]["progress"], json!({ "kind": "none" }));
}

// — `franchise-page-franchises`: the corpus franchises ——————————————

const CORPUS: &str = "franchise-page-franchises";

/// The page recorded for corpus anchor `id`. Every scenario of the suite
/// opens one anchor's page with the same inputs, so its first one names
/// the case: "… (`id`) · franchise `id`".
fn corpus(id: &str) -> Value {
    let all = super::scenarios::suites();
    let (_, suite) = all
        .iter()
        .find(|(n, _)| n == CORPUS)
        .unwrap_or_else(|| panic!("no suite {CORPUS}"));
    let tail = format!(" ({id}) · franchise {id}");
    let found: Vec<&Value> = suite
        .cases
        .iter()
        .filter(|c| c.name.ends_with(&tail))
        .map(|c| &c.answer)
        .collect();
    assert_eq!(found.len(), 1, "one recorded page for {id}");
    found[0]["page"].clone()
}

fn rail_has(p: &Value, rail: &str, id: &str) -> bool {
    ids(&p[rail]).contains(&id)
}

/// Every corpus anchor and the name its page carries.
const NAMES: &[(&str, &str)] = &[
    ("12", "One Piece"),
    ("50656", "One Piece"),
    ("50409", "One Piece"),
    ("45513", "One Piece"),
    ("428", "One Piece"),
    ("11", "Naruto"),
    ("1555", "Naruto"),
    ("13051", "Naruto"),
    ("10089", "Naruto"),
    ("7543", "Naruto"),
    ("244", "BLEACH"),
    ("43078", "BLEACH"),
    ("1515", "BLEACH"),
    ("199", "Dragon Ball"),
    ("720", "Dragon Ball"),
    ("10879", "Dragon Ball"),
    ("44544", "Dragon Ball"),
    ("210", "Detective Conan"),
    ("696", "Detective Conan"),
    ("818", "Gintama"),
    ("7241", "Gintama"),
    ("486", "Pocket Monsters"),
    ("1265", "Lupin III"),
    ("323", "Fate/stay night"),
    ("6028", "Fate/stay night"),
    ("6928", "Fate/stay night"),
    ("3919", "Bakemonogatari"),
    ("7158", "JoJo no Kimyou na Bouken (TV)"),
    ("7730", "JoJo no Kimyou na Bouken (TV)"),
    ("7442", "Attack on Titan"),
    ("46874", "SPY×FAMILY"),
    ("184", "R.O.D OVA"),
    ("185", "R.O.D OVA"),
    ("202", "FLCL"),
    ("11961", "FLCL"),
    ("6084", "Kenpuu Denki Berserk"),
    ("11655", "Kenpuu Denki Berserk"),
    ("115", "Hunter x Hunter"),
    ("1046", "Sakura Taisen: Ouka Kenran"),
    ("1162", "Kidou Keisatsu Patlabor"),
    ("2357", "Kara no Kyoukai 1: Fukan Fuukei"),
    ("7124", "Persona 3 the Movie 1: Spring of Birth"),
    ("841", "Top wo Nerae! Gunbuster"),
    ("14084", "Boku no Hero Academia"),
    ("11469", "Boku no Hero Academia"),
    ("13881", "Boku no Hero Academia"),
    ("42205", "Boku no Hero Academia"),
    ("47844", "Boku no Hero Academia"),
];

#[test]
fn every_corpus_franchise_page_carries_the_name_the_walk_gives_it() {
    for (id, name) in NAMES {
        assert_eq!(corpus(id)["name"], *name, "the page for {id}");
    }
}

#[test]
fn films_that_name_a_series_their_parent_sit_under_films_not_side_stories() {
    for id in ["1515", "45513", "46874", "696", "7543"] {
        let p = corpus(id);
        assert!(rail_has(&p, "films", id), "{id} under Films");
        assert!(!rail_has(&p, "sides", id), "{id} not a side story");
    }
}

#[test]
fn one_piece_is_its_series_alone_with_lego_one_piece_a_side_story_from_either_end() {
    for id in ["12", "50656"] {
        let p = corpus(id);
        assert_eq!(p["name"], "One Piece", "from {id}");
        assert_eq!(row_ids(&p), ["12"], "from {id}");
        assert!(rail_has(&p, "sides", "50656"), "from {id}");
    }
}

#[test]
fn naruto_lists_four_titles_by_their_whole_titles_and_boruto_s_film_under_films() {
    let p = corpus("11");
    assert_eq!(p["name"], "Naruto");
    assert_eq!(rows(&p).len(), 4);
    assert_eq!(
        row(&p, "13051")["entry"]["title"],
        "Boruto: Naruto Next Generations"
    );
    assert_eq!(
        row(&p, "47181")["entry"]["title"],
        "Boruto: Naruto Next Generations Part 2"
    );
    assert!(rail_has(&p, "films", "10089"));
    assert!(!rail_has(&p, "versions", "10089"));
}

#[test]
fn bleach_lists_five_titles_by_their_whole_titles() {
    let p = corpus("244");
    assert_eq!(rows(&p).len(), 5);
    assert_eq!(
        row(&p, "43078")["entry"]["title"],
        "BLEACH: Sennen Kessen-hen"
    );
    assert_eq!(
        row(&p, "46903")["entry"]["title"],
        "BLEACH: Sennen Kessen-hen - Ketsubetsu-tan"
    );
}

#[test]
fn dragon_ball_puts_its_films_under_films_beerus_beside_kai_and_lists_its_recaps() {
    let p = corpus("199");
    assert_eq!(p["name"], "Dragon Ball");
    assert!(!rows(&p).is_empty());
    assert!(rail_has(&p, "films", "461"));
    assert!(rail_has(&p, "films", "7176"));
    assert!(!rail_has(&p, "versions", "461"));
    assert!(rail_has(&p, "versions", "4394"));
    assert!(rail_has(&p, "versions", "50497"));
    assert!(!rail_has(&p, "recaps", "50497"));
    assert!(rail_has(&p, "recaps", "11468"));
}
