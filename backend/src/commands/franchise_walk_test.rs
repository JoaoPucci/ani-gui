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

/// From the 2000 OVA, the main story is the franchise's, as from a TV
/// part: the 2012 series names Stardust Crusaders its sequel, and the
/// OVA names the series its prequel, so both follow the series; they
/// are versions of each other, and Stardust Crusaders, in the series'
/// format, is kept. Both OVAs are other versions, the Phantom Blood
/// film a film.
#[test]
fn from_the_2000_jojo_ova_the_main_story_is_the_series() {
    let f = drive("7730", &jojo());
    assert_eq!(ids(&f.entries), ["7158", "8063", "8739"]);
    assert_eq!(f.links, [pair("7158", "8063"), pair("8063", "8739")]);
    assert_eq!(ids(&f.versions), ["610", "7730", "13306"]);
    assert_eq!(ids(&f.films), ["3136"]);
    assert!(f.sides.is_empty() && f.recaps.is_empty());
    assert!(f.parents.is_empty());
    assert!(f.complete);
    let tv = drive("8063", &jojo());
    assert_eq!((f.name, f.name_from), (tv.name, tv.name_from));
}

/// From a TV part, no member lists either OVA as a prequel or sequel:
/// both are other versions, not members. The film is a film.
#[test]
fn from_a_jojo_tv_part_the_ovas_are_other_versions() {
    let f = drive("8063", &jojo());
    assert_eq!(ids(&f.entries), ["7158", "8063", "8739"]);
    assert_eq!(f.links, [pair("7158", "8063"), pair("8063", "8739")]);
    assert_eq!(ids(&f.versions), ["610", "7730", "13306"]);
    assert_eq!(ids(&f.films), ["3136"]);
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
/// is the one added, and the films, other versions of it, are films. The prologue is
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
    assert!(f.versions.is_empty());
    assert_eq!(ids(&f.recaps), ["6906"]);
    assert_eq!(ids(&f.films), ["4788", "7873", "8675"]);
    assert_eq!(ids(&f.sides), ["6928"]);
    assert!(f.parents.is_empty());
}

/// From the UBW series, the main story is the walk from Fate/Zero, the
/// franchise's root, as from the 2nd season: the 2006 series, which
/// the UBW series lists as a version, is in it, though a walk from the
/// UBW series itself would leave it out by rule 3. The films listed as
/// versions are films.
#[test]
fn from_unlimited_blade_works_the_main_story_is_the_franchises() {
    let f = drive("7882", &fate());
    assert_eq!(
        ids(&f.entries),
        ["323", "6028", "7658", "9718", "7882", "10052"]
    );
    assert!(f.versions.is_empty());
    assert_eq!(ids(&f.films), ["4788", "7873", "8675"]);
}

/// Berserk as Kitsu relates it: the 1997 series, the three 2012 films,
/// and the 2016 series that names both the 1997 series and the third
/// film its prequel, with its second season.
fn berserk() -> Catalogue {
    let mut c = Catalogue::default();
    c.add("24", "TV", Some("1997-10-08"))
        .add("6084", "movie", Some("2012-02-04"))
        .add("6664", "movie", Some("2012-06-23"))
        .add("6665", "movie", Some("2013-02-01"))
        .add("11655", "TV", Some("2016-07-01"))
        .add("12569", "TV", Some("2017-04-07"));
    c.rel("24", "sequel", "11655")
        .rel("24", "alternative_version", "6084")
        .rel("24", "alternative_version", "6664")
        .rel("24", "alternative_version", "6665")
        .rel("6084", "alternative_version", "24")
        .rel("6084", "sequel", "6664")
        .rel("6664", "prequel", "6084")
        .rel("6664", "sequel", "6665")
        .rel("6665", "prequel", "6664")
        .rel("6665", "sequel", "11655")
        .rel("11655", "sequel", "12569")
        .rel("11655", "prequel", "24")
        .rel("11655", "prequel", "6665")
        .rel("12569", "prequel", "11655");
    c
}

/// Every Berserk page answers with one main story: the 2016 series
/// keeps the 1997 series, in its own format, over the third film among
/// its prequels, so the franchise starts at the 1997 series, and the
/// films, which a walk from the first film would make its main story,
/// are films.
#[test]
fn every_berserk_page_has_the_1997_series_main_story() {
    let c = berserk();
    for anchor in ["24", "6084", "6664", "6665", "11655", "12569"] {
        let f = drive(anchor, &c);
        assert_eq!(ids(&f.entries), ["24", "11655", "12569"], "from {anchor}");
        assert_eq!(f.name_from, "24", "from {anchor}");
        assert_eq!(ids(&f.films), ["6084", "6664", "6665"], "from {anchor}");
        assert!(f.parents.is_empty(), "from {anchor}");
    }
}

/// A prequel that names nothing back still leads the main story: the
/// link counts from the side that lists it.
#[test]
fn a_prequel_that_names_nothing_back_leads_the_main_story() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"))
        .add("2", "TV", Some("2002-01-01"));
    c.rel("2", "prequel", "1");
    let f = drive("2", &c);
    assert_eq!(ids(&f.entries), ["1", "2"]);
    assert_eq!(f.links, [pair("1", "2")]);
    assert_eq!(f.name_from, "1");
}

/// Two entries that name each other as prequel and sequel both ways
/// round have no first by links: the earlier stands for the root, from
/// either page.
#[test]
fn a_loop_of_links_has_one_main_story_from_either_side() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"))
        .add("2", "TV", Some("2002-01-01"));
    c.rel("1", "sequel", "2")
        .rel("1", "prequel", "2")
        .rel("2", "sequel", "1")
        .rel("2", "prequel", "1");
    let one = drive("1", &c);
    let two = drive("2", &c);
    assert_eq!(ids(&one.entries), ["1", "2"]);
    assert_eq!(one.entries, two.entries);
    assert_eq!(one.name_from, two.name_from);
}

/// A franchise past the listing bound beyond what the anchor's own run
/// reaches is read only to the bound, and the answer is incomplete.
#[test]
fn a_franchise_past_the_listing_bound_is_incomplete() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"))
        .add("2", "OVA", Some("2001-01-01"));
    c.rel("1", "sequel", "2")
        .rel("2", "alternative_version", "1");
    for i in 3..=120 {
        c.add(&i.to_string(), "OVA", Some(&format!("{}-01-01", 1900 + i)));
        c.rel(&(i - 1).to_string(), "sequel", &i.to_string());
    }
    let f = drive("1", &c);
    assert!(!f.complete);
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

/// A run holds one of each group of versions, even when the second is
/// met through another entry than the first: A lists P as its prequel
/// and S1 as its sequel, and P lists S2 as a sequel beside them. S1
/// and S2 are versions of each other, so S2, met a step later through
/// P, is left out although S1 is not on its path.
#[test]
fn a_version_of_a_member_met_through_another_entry_is_left_out() {
    let mut c = Catalogue::default();
    c.add("93001", "TV", Some("2010-01-01"))
        .add("93002", "TV", Some("2008-01-01"))
        .add("93003", "TV", Some("2012-01-01"))
        .add("93004", "TV", Some("2013-01-01"));
    c.rel("93001", "prequel", "93002")
        .rel("93001", "sequel", "93003")
        .rel("93002", "sequel", "93001")
        .rel("93002", "sequel", "93003")
        .rel("93002", "sequel", "93004")
        .rel("93003", "alternative_version", "93004");
    let f = drive("93001", &c);
    assert_eq!(ids(&f.entries), ["93002", "93001", "93003"]);
    assert_eq!(ids(&f.versions), ["93004"]);
}

/// Music is never listed around the run, and that holds for a music
/// entry rules 2 and 3 leave out: it is not an other version either.
#[test]
fn a_music_entry_left_out_is_not_an_other_version() {
    let mut c = Catalogue::default();
    c.add("94001", "TV", Some("2010-01-01"))
        .add("94002", "TV", Some("2011-01-01"))
        .add("94003", "music", Some("2011-01-01"));
    c.rel("94001", "sequel", "94002")
        .rel("94001", "sequel", "94003")
        .rel("94003", "alternative_version", "94002");
    let f = drive("94001", &c);
    assert_eq!(ids(&f.entries), ["94001", "94002"]);
    assert!(f.versions.is_empty(), "{:?}", ids(&f.versions));
}

/// Two versions met in one step through different entries still keep
/// one when an entry lists both: R lists X and Y, X lists A, and Y
/// lists A and B, which are versions of each other. B starts first, so
/// it is decided first, but A must not join beside it.
#[test]
fn versions_met_in_one_step_through_different_entries_keep_one() {
    let mut c = Catalogue::default();
    c.add("95001", "TV", Some("2010-01-01"))
        .add("95002", "TV", Some("2011-01-01"))
        .add("95003", "TV", Some("2011-06-01"))
        .add("95004", "TV", Some("2013-01-01"))
        .add("95005", "TV", Some("2012-01-01"));
    c.rel("95001", "sequel", "95002")
        .rel("95001", "sequel", "95003")
        .rel("95002", "sequel", "95004")
        .rel("95003", "sequel", "95004")
        .rel("95003", "sequel", "95005")
        .rel("95004", "alternative_version", "95005");
    let f = drive("95001", &c);
    let members = ids(&f.entries);
    assert!(
        members.contains(&"95004") != members.contains(&"95005"),
        "{members:?}"
    );
}

/// A named parent story whose listing names fewer entries beneath it
/// than the anchor's is not taken; but its listing was read to decide
/// that, and a listing cut short there makes the answer incomplete.
#[test]
fn a_rejected_parent_read_in_part_makes_it_incomplete() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("1999-10-20"))
        .add("2", "ONA", Some("2026-09-29"))
        .add("3", "special", Some("2005-01-01"))
        .add("4", "special", Some("2006-01-01"));
    c.rel("1", "parent_story", "2")
        .rel("1", "side_story", "3")
        .rel("1", "summary", "4")
        .rel("2", "side_story", "1");
    c.listings.get_mut("2").expect("listing").complete = false;
    let f = drive("1", &c);
    assert_eq!(ids(&f.entries), ["1"]);
    assert!(f.parents.is_empty());
    assert!(!f.complete);
}

/// An earlier side story that names more entries beneath it is the
/// anchor's parent only when it names the anchor back as its parent
/// story: one that does not is no sign of a pair listed the wrong way
/// round.
#[test]
fn an_earlier_side_story_that_does_not_name_the_anchor_back_is_not_its_parent() {
    let mut c = Catalogue::default();
    c.add("1", "OVA", Some("2010-01-01"))
        .add("2", "TV", Some("2000-01-01"))
        .add("3", "special", Some("2001-01-01"))
        .add("4", "special", Some("2002-01-01"));
    c.rel("1", "side_story", "2")
        .rel("2", "side_story", "3")
        .rel("2", "summary", "4");
    let f = drive("1", &c);
    assert_eq!(ids(&f.entries), ["1"]);
    assert!(f.parents.is_empty());
    // Named back, it is the parent.
    c.rel("2", "parent_story", "1");
    let f = drive("1", &c);
    assert_eq!(ids(&f.entries), ["2"]);
    assert_eq!(f.parents.get("1").cloned(), Some(vec!["2".to_string()]));
}

/// A named parent story that started before the anchor is its parent
/// even when the anchor names more entries beneath it: a side story
/// with recaps of its own is still a side story. Only a parent that
/// started after the anchor and names fewer is taken as the pair
/// listed the wrong way round.
#[test]
fn a_parent_that_started_first_is_the_parent_whatever_it_names() {
    let mut c = Catalogue::default();
    c.add("1", "OVA", Some("2010-01-01"))
        .add("2", "TV", Some("2005-01-01"))
        .add("3", "special", Some("2011-01-01"))
        .add("4", "special", Some("2012-01-01"));
    c.rel("1", "parent_story", "2")
        .rel("1", "summary", "3")
        .rel("1", "summary", "4")
        .rel("2", "side_story", "1");
    let f = drive("1", &c);
    assert_eq!(ids(&f.entries), ["2"]);
    assert_eq!(f.parents.get("1").cloned(), Some(vec!["2".to_string()]));
}
