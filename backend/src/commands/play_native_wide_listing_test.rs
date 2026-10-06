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

#[test]
fn a_listing_spanning_two_entries_is_cut_to_the_first() {
    // hianime's Season 3 holds Kitsu's Season 3 (12) and Part 2 (10).
    let wide = hit("aot-s3-866", "Attack on Titan Season 3");
    let later = hit("aot-s3p2-1895", "Attack on Titan Season 3 Part 2");
    let mut probed = pool(&[(&wide, 22, true), (&later, 10, false)], 12);
    fit_to_entry(
        &mut probed,
        12,
        EntryTitles::bare("Attack on Titan Season 3"),
    );
    assert_eq!(probed[0].2, 0, "the cut listing is the exact fit");
    assert_eq!(
        probed[0].1,
        listing(12),
        "episodes 1..12, numbered as listed"
    );
    assert_eq!(probed[1].2, UNFIT, "the later part is the next entry");
}

#[test]
fn the_later_part_may_name_its_part_in_any_words_after_the_stem() {
    // Gintama's Silver Soul Arc: 26 listed, its second half (14)
    // named with no marker at all.
    let wide = hit("gintama-ss-1155", "Gintama.: Silver Soul Arc");
    let later = hit(
        "gintama-ss2-1157",
        "Gintama.: Silver Soul Arc - Second Half War",
    );
    let mut probed = pool(&[(&later, 14, true), (&wide, 26, true)], 12);
    fit_to_entry(
        &mut probed,
        12,
        EntryTitles::bare("Gintama.: Shirogane no Tamashii-hen"),
    );
    assert_eq!((probed[1].2, probed[1].1.len()), (0, 12));
    assert_eq!(probed[0].2, UNFIT);
}

#[test]
fn nothing_spans_without_the_sibling_that_completes_it() {
    // A later part one episode off the remainder is no proof, nor is
    // a listing of the right length that does not share the stem.
    let wide = hit("aot-s3-866", "Attack on Titan Season 3");
    let near = hit("aot-s3p2-1895", "Attack on Titan Season 3 Part 2");
    let other = hit("mob-926", "Mob Psycho 100");
    let e = EntryTitles::bare("Attack on Titan Season 3");
    let mut probed = pool(
        &[(&wide, 22, true), (&near, 9, false), (&other, 10, false)],
        12,
    );
    fit_to_entry(&mut probed, 12, e);
    assert_eq!(probed[0].1.len(), 22, "uncut");
    assert_eq!(probed[0].2, 10);
}

#[test]
fn a_spanning_listing_needs_the_entry_s_own_year() {
    let wide = hit("aot-s3-866", "Attack on Titan Season 3");
    let later = hit("aot-s3p2-1895", "Attack on Titan Season 3 Part 2");
    let mut probed = pool(&[(&wide, 22, false), (&later, 10, false)], 12);
    fit_to_entry(
        &mut probed,
        12,
        EntryTitles::bare("Attack on Titan Season 3"),
    );
    assert_eq!(
        probed[0].1.len(),
        22,
        "a tolerated year does not vouch for a cut"
    );
}

#[test]
fn a_dedicated_listing_that_fits_is_preferred_to_cutting() {
    let wide = hit("gintama-ss-1155", "Gintama.: Silver Soul Arc");
    let later = hit(
        "gintama-ss2-1157",
        "Gintama.: Silver Soul Arc - Second Half War",
    );
    let own = hit(
        "gintama-ss1-1156",
        "Gintama.: Silver Soul Arc - First Half War",
    );
    let e = EntryTitles::bare("Gintama.: Shirogane no Tamashii-hen");
    let mut probed = pool(
        &[(&wide, 26, true), (&later, 14, true), (&own, 12, true)],
        12,
    );
    fit_to_entry(&mut probed, 12, e);
    assert_eq!(
        probed[0].1.len(),
        26,
        "uncut: the entry has a listing of its own"
    );
    assert_eq!(probed[2].2, 0);
}

#[test]
fn the_cut_keeps_a_recap_listed_among_the_entry_s_episodes() {
    // hianime numbers a recap by its slot and tags it with the site's
    // number; the regular rows after it carry their own as tags.
    let row = |number: u32, tag: Option<&str>| EpisodeRef {
        id: u64::from(number),
        number,
        number2: tag.map(str::to_string),
    };
    let rows = vec![
        row(1, None),
        row(2, None),
        row(3, Some("2.5")),
        row(4, Some("3")),
        row(5, Some("4")),
    ];
    let head = head_of(&rows, 3);
    assert_eq!(head, rows[..4].to_vec(), "1, 2, the 2.5 recap, 3");
}

proptest::proptest! {
    /// The cut over arbitrary listings: a prefix of the listing, with
    /// exactly `expected` regular episodes when the listing has that
    /// many, and the whole listing when it has fewer.
    #[test]
    fn the_cut_is_the_prefix_holding_the_entry_s_episodes(
        tags in proptest::collection::vec(proptest::bool::ANY, 0..30),
        expected in 0u32..20,
    ) {
        let rows: Vec<EpisodeRef> = tags
            .iter()
            .enumerate()
            .map(|(i, recap)| EpisodeRef {
                id: i as u64,
                number: i as u32 + 1,
                number2: recap.then(|| format!("{i}.5")),
            })
            .collect();
        let head = head_of(&rows, expected);
        proptest::prop_assert_eq!(&rows[..head.len()], &head[..]);
        let regular = regular_episode_count(&head);
        let total = regular_episode_count(&rows);
        proptest::prop_assert_eq!(regular, expected.min(total));
    }
}
