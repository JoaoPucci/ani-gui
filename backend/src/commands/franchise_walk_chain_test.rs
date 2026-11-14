//! Worked cases for a page's own chain (`franchise_walk_chain.rs`).

use super::tests::{drive, Catalogue};
use super::*;

fn ids(list: &[FranchiseEntry]) -> Vec<String> {
    list.iter().map(|e| e.id.clone()).collect()
}

/// A TV line (1 → 2) and a film line (3 → 4) that follows 1, film 3 a
/// version of 2: the series keeps 2, and the films are a chain of their
/// own after 1.
fn films_after_a_series() -> Catalogue {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"))
        .add("2", "TV", Some("2002-01-01"))
        .add("3", "movie", Some("2003-01-01"))
        .add("4", "movie", Some("2004-01-01"))
        .rel("1", "sequel", "2")
        .rel("1", "sequel", "3")
        .rel("2", "prequel", "1")
        .rel("2", "alternative_version", "3")
        .rel("3", "prequel", "1")
        .rel("3", "alternative_version", "2")
        .rel("3", "sequel", "4")
        .rel("4", "prequel", "3");
    c
}

/// A side story of a chain title keeps it as its parent, and is drawn
/// on that title's chain: its parent is not lost because the main
/// story leaves it out.
#[test]
fn a_side_story_of_a_chain_title_keeps_its_parent_on_the_chain() {
    let mut c = films_after_a_series();
    c.add("9", "OVA", Some("2005-01-01"))
        .rel("3", "side_story", "9")
        .rel("9", "parent_story", "3");
    let f = drive("9", &c);
    assert_eq!(ids(&f.entries), ["1", "2"]);
    assert_eq!(f.parents.get("9").cloned(), Some(vec!["3".to_string()]));
    assert!(ids(&f.sides).contains(&"9".to_string()));
    let chain = f.chain.expect("the parent's chain");
    assert_eq!(ids(&chain.entries), ["1", "3", "4"]);
    assert_eq!(chain.seams, ["1"]);
}

/// A music video joined to a chain by a sequel link is no title of it,
/// as it is no member of a run.
#[test]
fn a_music_video_is_no_title_of_a_chain() {
    let mut c = films_after_a_series();
    c.add("7", "music", Some("2006-01-01"))
        .rel("4", "sequel", "7")
        .rel("7", "prequel", "4");
    let f = drive("3", &c);
    let chain = f.chain.expect("a chain");
    assert_eq!(ids(&chain.entries), ["1", "3", "4"]);
}

/// Seams are listed in start order, then numeric id, as every other
/// list is: id 11 after id 9.
#[test]
fn seams_are_in_start_then_numeric_order() {
    let mut c = Catalogue::default();
    c.add("9", "TV", Some("2000-01-01"))
        .add("10", "TV", Some("2002-01-01"))
        .add("11", "TV", Some("2004-01-01"))
        .add("30", "movie", Some("2001-01-01"))
        .add("31", "movie", Some("2003-01-01"))
        .rel("9", "sequel", "10")
        .rel("9", "sequel", "30")
        .rel("10", "prequel", "9")
        .rel("10", "sequel", "11")
        .rel("10", "alternative_version", "30")
        .rel("10", "alternative_version", "31")
        .rel("11", "prequel", "10")
        .rel("11", "prequel", "31")
        .rel("30", "prequel", "9")
        .rel("30", "alternative_version", "10")
        .rel("30", "sequel", "31")
        .rel("31", "prequel", "30")
        .rel("31", "sequel", "11")
        .rel("31", "alternative_version", "10");
    let f = drive("30", &c);
    assert_eq!(ids(&f.entries), ["9", "10", "11"]);
    let chain = f.chain.expect("a chain");
    assert_eq!(chain.seams, ["9", "11"]);
}

/// A page that lists one entry as both its prequel and its other
/// version is not on a chain through it: the two are versions of one
/// another, and a version is never the next title of the chain it
/// retells, so the page gets no seam into its own version.
#[test]
fn a_page_is_not_chained_to_its_own_version() {
    let mut c = Catalogue::default();
    c.add("1", "OVA", Some("2005-01-01"))
        .add("2", "TV", Some("2000-01-01"))
        .rel("1", "prequel", "2")
        .rel("1", "alternative_version", "2");
    let f = drive("1", &c);
    assert_eq!(ids(&f.entries), ["2"]);
    assert_eq!(f.chain, None);
}

/// Two titles outside the main story that list each other as sequel
/// and as other version are not chained either: the chain stops before
/// the version, which is no title of it.
#[test]
fn a_chain_does_not_go_on_into_a_version_of_its_title() {
    let mut c = films_after_a_series();
    c.add("5", "movie", Some("2005-01-01"))
        .rel("4", "sequel", "5")
        .rel("5", "prequel", "4")
        .rel("5", "alternative_version", "4");
    let f = drive("3", &c);
    let chain = f.chain.expect("a chain");
    assert_eq!(ids(&chain.entries), ["1", "3", "4"]);
    assert!(chain.links.iter().all(|(a, b)| a != "5" && b != "5"));
    assert_eq!(drive("5", &c).chain, None);
}
