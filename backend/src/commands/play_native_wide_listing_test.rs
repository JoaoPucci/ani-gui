use super::*;

fn hit(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
    }
}

fn listing(n: u32) -> Vec<EpisodeRef> {
    (1..=n)
        .map(|k| EpisodeRef {
            id: u64::from(k),
            number: k,
            number2: None,
        })
        .collect()
}

/// The pool as the probe loop leaves it: listing, count distance and
/// the year flag per hit.
fn pool<'h>(rows: &[(&'h BrowseHit, u32, bool)], expected: u32) -> Vec<Probed<'h>> {
    rows.iter()
        .map(|(h, n, confirmed)| (*h, listing(*n), n.abs_diff(expected), *confirmed))
        .collect()
}

#[test]
fn a_candidate_the_entry_does_not_admit_is_scored_out_but_kept() {
    let own = hit("oshi-675", "My Star");
    let sequel = hit("oshi-s2-682", "My Star: Season 2");
    let mut probed = pool(&[(&sequel, 13, false), (&own, 11, true)], 11);
    fit_to_entry(&mut probed, 11, EntryTitles::bare("[Oshi no Ko]"));
    assert_eq!(probed[0].2, UNFIT);
    assert_eq!(
        probed[0].1.len(),
        13,
        "its listing stays for any rule that reads it"
    );
    assert_eq!(probed[1].2, 0);
}
