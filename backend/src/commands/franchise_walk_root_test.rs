//! Worked cases for the franchise's root (`franchise_walk_root.rs`) and
//! the graph it is found in (`franchise_walk_graph.rs`), each a choice
//! the walk's other tests reach only through the whole walk.

use std::collections::BTreeSet;

use super::super::graph::{neighbours, Graph};
use super::super::prequels::prequels;
use super::super::tests::Catalogue;
use super::super::Lookup;
use super::root;

fn ids(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

/// The whole catalogue as one franchise, every listing read.
fn whole(cat: &Catalogue) -> Graph {
    let all: BTreeSet<String> = cat.listings.keys().cloned().collect();
    Graph {
        entries: all.clone(),
        consulted: all,
        complete: true,
    }
}

fn look(cat: &Catalogue) -> Lookup<'_> {
    Lookup {
        known: &cat.listings,
        details: &cat.entries,
    }
}

/// A sequel whose listing names its prequel, the prequel's listing not
/// naming it back, is the prequel's neighbour once its listing is read,
/// and not before.
#[test]
fn a_link_listed_on_one_side_counts_once_that_side_is_read() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"))
        .add("2", "OVA", Some("2001-01-01"))
        .rel("2", "prequel", "1");
    let l = look(&c);
    assert_eq!(neighbours("1", &l, &ids(&["1", "2"])), ids(&["2"]));
    assert_eq!(neighbours("1", &l, &ids(&["1"])), BTreeSet::new());
}

/// Two prequels that are versions of one another: the one in the
/// listing entry's format is followed back, though the other started
/// earlier.
#[test]
fn a_prequel_in_the_entrys_own_format_is_followed() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2000-01-01"))
        .add("2", "movie", Some("2005-01-01"))
        .add("3", "movie", Some("2008-01-01"))
        .rel("3", "prequel", "1")
        .rel("3", "prequel", "2")
        .rel("1", "alternative_version", "2");
    let g = whole(&c);
    let (followed, passed) = prequels("3", &look(&c), &g);
    assert_eq!(followed, vec!["2".to_string()]);
    assert_eq!(passed, vec!["1".to_string()]);
    assert_eq!(root(&look(&c), &g).as_deref(), Some("2"));
}

/// An entry passed over where two lines meet is no root, though it
/// started first, nothing comes before it, and another line's way back
/// reaches it.
#[test]
fn an_entry_passed_over_is_never_the_root() {
    let mut c = Catalogue::default();
    c.add("1", "movie", Some("1990-01-01"))
        .add("2", "TV", Some("1995-01-01"))
        .add("3", "TV", Some("2000-01-01"))
        .add("4", "OVA", Some("1992-01-01"))
        .rel("3", "prequel", "1")
        .rel("3", "prequel", "2")
        .rel("1", "alternative_version", "2")
        .rel("1", "sequel", "3")
        .rel("2", "sequel", "3")
        .rel("4", "prequel", "1")
        .rel("1", "sequel", "4");
    assert_eq!(root(&look(&c), &whole(&c)).as_deref(), Some("2"));
}
