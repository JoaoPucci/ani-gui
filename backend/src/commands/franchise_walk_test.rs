//! Worked cases for the franchise walk, from the listings the proposal
//! (`docs/proposals/franchise-watch-order.md`, sections 2 and 4) reads.
//! JoJo's and Fate's ids and dates are Kitsu's; Dragon Ball's and most
//! of Gintama's entries are stand-ins with the shape Kitsu gives them.

use std::collections::BTreeMap;

use super::*;
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, FranchiseRelation};

/// One entry of a hand-built catalogue.
pub(super) fn entry(id: &str, subtype: &str, start: Option<&str>) -> FranchiseEntry {
    FranchiseEntry {
        id: id.into(),
        title: format!("T{id}"),
        subtype: Some(subtype.into()),
        start: start.map(str::to_string),
        episodes: Some(12),
        status: Some("finished".into()),
        poster: None,
        cover: None,
    }
}

/// A catalogue: every entry and what its listing relates it to.
#[derive(Debug, Default, Clone)]
pub(super) struct Catalogue {
    pub(super) entries: BTreeMap<String, FranchiseEntry>,
    pub(super) listings: BTreeMap<String, FranchiseListing>,
}

impl Catalogue {
    pub(super) fn add(&mut self, id: &str, subtype: &str, start: Option<&str>) -> &mut Self {
        self.entries.insert(id.into(), entry(id, subtype, start));
        self.listings.entry(id.into()).or_insert(FranchiseListing {
            relations: Vec::new(),
            complete: true,
        });
        self
    }

    /// `from` lists `to` under `role`. Both must be added first.
    pub(super) fn rel(&mut self, from: &str, role: &str, to: &str) -> &mut Self {
        let to_entry = self.entries[to].clone();
        self.listings
            .get_mut(from)
            .expect("listing")
            .relations
            .push(FranchiseRelation {
                role: role.into(),
                kind: "anime".into(),
                id: to.into(),
                subtype: to_entry.subtype.clone(),
                entry: Some(to_entry),
            });
        self
    }

    fn manga(&mut self, from: &str, role: &str, to: &str) -> &mut Self {
        self.listings
            .get_mut(from)
            .expect("listing")
            .relations
            .push(FranchiseRelation {
                role: role.into(),
                kind: "manga".into(),
                id: to.into(),
                subtype: Some("manga".into()),
                entry: None,
            });
        self
    }
}

/// Run the walk the way the IO layer does, reading from `cat`.
pub(super) fn drive(anchor: &str, cat: &Catalogue) -> Franchise {
    let mut known = BTreeMap::new();
    let mut details = BTreeMap::new();
    for _ in 0..500 {
        match plan(anchor, &known, &details) {
            Plan::Done(f) => return f,
            Plan::Need {
                listings,
                details: d,
            } => {
                assert!(
                    !listings.is_empty() || !d.is_empty(),
                    "a need with nothing in it"
                );
                for id in listings {
                    assert!(!known.contains_key(&id), "asked again for {id}");
                    known.insert(id.clone(), cat.listings[&id].clone());
                }
                for id in d {
                    assert!(!details.contains_key(&id), "asked again for {id}");
                    details.insert(id.clone(), cat.entries[&id].clone());
                }
            }
        }
    }
    panic!("the walk did not finish");
}

fn ids(entries: &[FranchiseEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.id.as_str()).collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

/// JoJo as Kitsu relates it: the 2012 series, both Stardust Crusaders
/// seasons, the 2000 and 1993 OVAs, the 2007 film and the re-edit.
fn jojo() -> Catalogue {
    let mut c = Catalogue::default();
    c.add("7158", "TV", Some("2012-10-06"))
        .add("8063", "TV", Some("2014-04-05"))
        .add("8739", "TV", Some("2015-01-10"))
        .add("7730", "OVA", Some("2000-05-25"))
        .add("610", "OVA", Some("1993-11-19"))
        .add("3136", "movie", Some("2007-02-17"))
        .add("13306", "special", Some("2012-12-31"));
    c.rel("7158", "sequel", "8063")
        .rel("7158", "alternative_version", "3136")
        .rel("7158", "alternative_version", "13306")
        .rel("8063", "prequel", "7158")
        .rel("8063", "sequel", "8739")
        .rel("8063", "alternative_version", "7730")
        .rel("8739", "prequel", "8063")
        .rel("8739", "alternative_version", "610")
        .rel("7730", "sequel", "610")
        .rel("7730", "alternative_version", "8063")
        .rel("7730", "prequel", "7158")
        .manga("7730", "adaptation", "1899")
        .rel("610", "alternative_version", "8739")
        .rel("610", "prequel", "7730");
    c
}

/// From the 2000 OVA, the run is the 2012 series → the 2000 OVA → the
/// 1993 OVA: Stardust Crusaders lists the OVA as another version, so
/// rule 3 leaves it out, and it is listed with the rest of the versions.
#[test]
fn from_the_2000_jojo_ova_the_run_is_the_ovas_after_the_2012_series() {
    let f = drive("7730", &jojo());
    assert_eq!(ids(&f.entries), ["610", "7730", "7158"]);
    assert_eq!(f.links, [pair("7158", "7730"), pair("7730", "610")]);
    assert_eq!(ids(&f.versions), ["3136", "13306", "8063", "8739"]);
    assert!(f.films.is_empty() && f.sides.is_empty() && f.recaps.is_empty());
    assert!(f.parents.is_empty());
    assert!(f.complete);
}

/// From a TV part, no member lists either OVA as a prequel or sequel:
/// both are other versions, not members.
#[test]
fn from_a_jojo_tv_part_the_ovas_are_other_versions() {
    let f = drive("8063", &jojo());
    assert_eq!(ids(&f.entries), ["7158", "8063", "8739"]);
    assert_eq!(f.links, [pair("7158", "8063"), pair("8063", "8739")]);
    assert_eq!(ids(&f.versions), ["610", "7730", "3136", "13306"]);
}

/// Fate as Kitsu relates it around Fate/Zero, with a same-world series
/// (a stand-in) listed as an alternative setting.
fn fate() -> Catalogue {
    let mut c = Catalogue::default();
    c.add("6028", "TV", Some("2011-10-02"))
        .add("7658", "TV", Some("2012-04-08"))
        .add("6928", "special", Some("2012-03-07"))
        .add("6906", "special", Some("2012-03-25"))
        .add("7873", "movie", Some("2013-07-13"))
        .add("42092", "TV", Some("2019-07-07"))
        .add("323", "TV", Some("2006-01-07"))
        .add("4788", "movie", Some("2010-01-23"))
        .add("8675", "movie", Some("2017-10-14"))
        .add("9718", "special", Some("2014-10-05"))
        .add("7882", "TV", Some("2014-10-12"))
        .add("10052", "TV", Some("2015-04-05"))
        .add("90001", "TV", Some("2017-07-02"));
    c.rel("6028", "other", "7873")
        .rel("6028", "summary", "6906")
        .rel("6028", "sequel", "7658")
        .rel("6028", "side_story", "6928")
        .rel("6028", "character", "42092")
        .rel("7658", "prequel", "6028")
        .rel("7658", "side_story", "6928")
        .rel("7658", "sequel", "323")
        .rel("7658", "sequel", "4788")
        .rel("7658", "sequel", "8675")
        .rel("7658", "sequel", "9718")
        .rel("6928", "parent_story", "6028")
        .rel("6928", "parent_story", "7658")
        .rel("323", "prequel", "7658")
        .rel("323", "alternative_version", "4788")
        .rel("323", "alternative_version", "8675")
        .rel("323", "alternative_version", "7882")
        .rel("323", "alternative_setting", "90001")
        .rel("4788", "alternative_version", "323")
        .rel("4788", "alternative_version", "7882")
        .rel("8675", "alternative_version", "323")
        .rel("9718", "prequel", "7658")
        .rel("9718", "sequel", "7882")
        .rel("7882", "alternative_version", "323")
        .rel("7882", "alternative_version", "4788")
        .rel("7882", "alternative_version", "8675")
        .rel("7882", "prequel", "9718")
        .rel("7882", "sequel", "10052")
        .rel("10052", "prequel", "7882");
    c
}

/// Fate/Zero 2nd Season lists four sequels. The 2006 series and the two
/// films list one another as versions: the series, in the same format,
/// is the one added, and the films are other versions. The prologue is
/// no version of them, so the run forks after the 2nd season. The
/// same-world series is listed nowhere; the recap, the film and the
/// side story sit around the run.
#[test]
fn fate_zero_forks_and_the_same_format_series_wins_its_group() {
    let f = drive("7658", &fate());
    assert_eq!(
        ids(&f.entries),
        ["323", "6028", "7658", "9718", "7882", "10052"]
    );
    assert!(f.links.contains(&pair("7658", "323")));
    assert!(f.links.contains(&pair("7658", "9718")));
    assert_eq!(ids(&f.versions), ["4788", "8675"]);
    assert_eq!(ids(&f.recaps), ["6906"]);
    assert_eq!(ids(&f.films), ["7873"]);
    assert_eq!(ids(&f.sides), ["6928"]);
    assert!(f.parents.is_empty());
}

/// From the UBW series, the 2006 series it lists as a version is left
/// out of the run by rule 3 and listed as a version.
#[test]
fn from_unlimited_blade_works_the_2006_series_is_a_version() {
    let f = drive("7882", &fate());
    assert_eq!(ids(&f.entries), ["6028", "7658", "9718", "7882", "10052"]);
    assert_eq!(ids(&f.versions), ["323", "4788", "8675"]);
}

/// A side story with two parents walks from the earlier parent; it is
/// listed among the sides, with both parents named.
#[test]
fn a_side_story_with_two_parents_names_both() {
    let f = drive("6928", &fate());
    assert_eq!(f.anchor, "6928");
    assert_eq!(
        ids(&f.entries),
        ["323", "6028", "7658", "9718", "7882", "10052"]
    );
    assert_eq!(ids(&f.sides), ["6928"]);
    assert_eq!(
        f.parents,
        BTreeMap::from([(
            "6928".to_string(),
            vec!["6028".to_string(), "7658".to_string()]
        )])
    );
}

/// Dragon Ball lists Z and Kai as sequels; Kai lists Z as another
/// version. From Z, Kai is met through Dragon Ball with Z on its path:
/// rule 3 leaves it out.
#[test]
fn dragon_ball_kai_is_left_out_along_the_path() {
    let mut c = Catalogue::default();
    c.add("91001", "TV", Some("1986-02-26"))
        .add("91002", "TV", Some("1989-04-26"))
        .add("91003", "TV", Some("2009-04-05"));
    c.rel("91001", "sequel", "91002")
        .rel("91001", "sequel", "91003")
        .rel("91002", "prequel", "91001")
        .rel("91003", "alternative_version", "91002");
    let f = drive("91002", &c);
    assert_eq!(ids(&f.entries), ["91001", "91002"]);
    assert_eq!(ids(&f.versions), ["91003"]);
}

/// Gintama's chain ends at The Final, which lists both the Silver Soul
/// finale and The Semi-Final as prequels: a join of two links.
#[test]
fn gintama_joins_at_the_final() {
    let mut c = Catalogue::default();
    c.add("818", "TV", Some("2006-04-04"))
        .add("92001", "TV", Some("2011-04-04"))
        .add("92002", "TV", Some("2018-07-09"))
        .add("92003", "movie", Some("2021-01-08"))
        .add("92004", "special", Some("2021-01-15"));
    c.rel("818", "sequel", "92001")
        .rel("92001", "prequel", "818")
        .rel("92001", "sequel", "92002")
        .rel("92002", "sequel", "92003")
        .rel("92003", "prequel", "92002")
        .rel("92003", "prequel", "92004")
        .rel("92004", "sequel", "92003");
    let f = drive("818", &c);
    assert_eq!(ids(&f.entries), ["818", "92001", "92002", "92003", "92004"]);
    assert!(f.links.contains(&pair("92002", "92003")));
    assert!(f.links.contains(&pair("92004", "92003")));
}

/// An entry with no relations is a run of itself alone.
#[test]
fn an_entry_with_no_relations_is_a_run_of_one() {
    let mut c = Catalogue::default();
    c.add("99", "TV", None);
    let f = drive("99", &c);
    assert_eq!(ids(&f.entries), ["99"]);
    assert!(f.links.is_empty() && f.versions.is_empty() && f.sides.is_empty());
    assert!(f.parents.is_empty());
    assert!(f.complete);
}

/// A run longer than the bound stops at it and is incomplete.
#[test]
fn a_run_past_the_bound_stops_there() {
    let mut c = Catalogue::default();
    for i in 1..=60 {
        c.add(&i.to_string(), "TV", Some(&format!("{}-01-01", 1950 + i)));
    }
    for i in 1..60 {
        c.rel(&i.to_string(), "sequel", &(i + 1).to_string());
    }
    let f = drive("1", &c);
    assert_eq!(f.entries.len(), MAX_MEMBERS);
    assert!(!f.complete);
}

/// An entry listing more sequels than the listing bound reads only as
/// many and is incomplete.
#[test]
fn a_walk_past_the_listing_bound_stops_there() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"));
    for i in 2..=120 {
        c.add(&i.to_string(), "movie", Some("2001-01-01"));
        c.rel("1", "sequel", &i.to_string());
    }
    let f = drive("1", &c);
    assert!(f.entries.len() <= MAX_MEMBERS);
    assert!(!f.complete);
}

/// A listing cut short by the page cap makes the franchise incomplete.
#[test]
fn a_cut_listing_makes_it_incomplete() {
    let mut c = jojo();
    c.listings.get_mut("8739").expect("listing").complete = false;
    assert!(!drive("8063", &c).complete);
}
