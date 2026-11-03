//! The alternative-version and alternative-setting links rules 2 and 3
//! read, and the relations an entry lists under given roles.

use super::Lookup;
use crate::meta::kitsu::FranchiseRelation;

const ALTERNATIVES: [&str; 2] = ["alternative_version", "alternative_setting"];

/// Whether `a`'s listing names `b` as an alternative version or setting.
fn lists_alt(look: &Lookup<'_>, a: &str, b: &str) -> bool {
    look.listing(a).is_some_and(|l| {
        l.relations
            .iter()
            .any(|r| r.id == b && r.kind == "anime" && ALTERNATIVES.contains(&r.role.as_str()))
    })
}

/// Whether `a` and `b` list each other, either way, as alternatives.
pub(crate) fn alt(look: &Lookup<'_>, a: &str, b: &str) -> bool {
    lists_alt(look, a, b) || lists_alt(look, b, a)
}

/// The anime `listing_of` lists under `role`, itself excepted.
pub(crate) fn listed<'a>(
    look: &'a Lookup<'_>,
    listing_of: &'a str,
    roles: &'a [&'a str],
) -> impl Iterator<Item = &'a FranchiseRelation> + 'a {
    look.listing(listing_of)
        .into_iter()
        .flat_map(|l| &l.relations)
        .filter(move |r| {
            r.kind == "anime" && r.id != listing_of && roles.contains(&r.role.as_str())
        })
}

/// The groups of `ids` joined, directly or through one another, by an
/// alternative link.
pub(super) fn components(look: &Lookup<'_>, ids: &[String]) -> Vec<Vec<String>> {
    let mut left: Vec<String> = ids.to_vec();
    let mut out = Vec::new();
    while let Some(first) = left.pop() {
        let mut group = vec![first];
        let mut i = 0;
        while i < group.len() {
            let (joined, rest): (Vec<String>, Vec<String>) =
                left.into_iter().partition(|c| alt(look, c, &group[i]));
            group.extend(joined);
            left = rest;
            i += 1;
        }
        out.push(group);
    }
    out
}
