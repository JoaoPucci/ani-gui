//! Properties of the special rule: it files a special exactly as a
//! model written from the documented rule does; a special it files
//! completes its entry only at the entry's last slot and writes nowhere
//! but that entry; and
//! weighing it against the other rules' map is the documented table.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use super::*;
use crate::account::episode_map::{Identity, Segment, Shape, Target};
use proptest::prelude::*;

/// A date as a source could send it, in one year so months meet: whole
/// days, months alone, impossible days, or none.
fn date() -> BoxedStrategy<Option<String>> {
    prop_oneof![
        1 => Just(None),
        6 => (1u32..=12, 1u32..=28).prop_map(|(m, d)| Some(format!("2015-{m:02}-{d:02}"))),
        1 => (1u32..=12).prop_map(|m| Some(format!("2015-{m:02}"))),
        1 => Just(Some("2015-02-30".to_string())),
    ]
    .boxed()
}

fn shape() -> BoxedStrategy<Shape> {
    prop_oneof![
        6 => Just(Shape::Extras),
        1 => Just(Shape::SplitParts),
        1 => Just(Shape::Identity(Identity::Conflict)),
        1 => Just(Shape::Identity(Identity::CountsAgree)),
    ]
    .boxed()
}

/// A related entry: its season's dates, its tracker entry, and a map of
/// one segment or none that mostly reads extras over a season of the
/// entry's count less them, sometimes off in one way.
fn filing() -> BoxedStrategy<RelatedFiling> {
    (
        (1u32..=3, 5u32..=24, prop::collection::vec(0u32..=6, 0..=3)),
        (date(), date(), shape(), any::<bool>(), date(), date()),
        (
            prop_oneof![6 => Just(0u32), 1 => 1u32..=3],
            prop_oneof![6 => Just(1u32), 1 => 2u32..=3],
            prop_oneof![8 => Just(false), 1 => Just(true)],
        ),
    )
        .prop_map(
            |(
                (id, season, mut after),
                (start, end, shape, finished, season_start, season_end),
                (lead, from, elsewhere),
            )| {
                after.sort_unstable();
                let total = season + u32::try_from(after.len()).unwrap_or(0);
                let entry = TrackerShow {
                    id,
                    id_mal: None,
                    episodes: Some(total),
                    finished,
                    start,
                    end,
                };
                let map = EpisodeMap {
                    shape,
                    segments: vec![Segment {
                        entry: if elsewhere { id + 10 } else { id },
                        entry_total: total,
                        entry_finished: finished,
                        kitsu_from: from,
                        kitsu_to: season,
                        lead,
                        extras_after: after,
                    }],
                };
                RelatedFiling {
                    related: KitsuShow {
                        id: format!("k{id}"),
                        episodes: Some(season),
                        start: season_start,
                        end: season_end,
                    },
                    entry,
                    map,
                    contested: false,
                }
            },
        )
        .boxed()
}

/// Evidence built so the rule fires, one field then sometimes knocked
/// off: a special of `k` episodes dated in the run of an entry whose
/// season's map reads `k` extras after one Kitsu episode.
fn firing_evidence() -> BoxedStrategy<SpecialEvidence> {
    (1u32..=3, 0u32..=5, 6u32..=24, 0u8..=11)
        .prop_map(|(k, w, season, knock)| {
            let total = season + k;
            let mut f = RelatedFiling {
                related: KitsuShow {
                    id: "k1".into(),
                    episodes: Some(season),
                    start: Some("2015-03-09".into()),
                    end: Some("2015-06-20".into()),
                },
                entry: TrackerShow {
                    id: 1,
                    id_mal: None,
                    episodes: Some(total),
                    finished: true,
                    start: Some("2015-01-05".into()),
                    end: Some("2015-06-20".into()),
                },
                map: EpisodeMap {
                    shape: Shape::Extras,
                    segments: vec![Segment {
                        entry: 1,
                        entry_total: total,
                        entry_finished: true,
                        kitsu_from: 1,
                        kitsu_to: season,
                        lead: 0,
                        extras_after: vec![w; k as usize],
                    }],
                },
                contested: false,
            };
            let mut special = KitsuShow {
                id: "s".into(),
                episodes: Some(k),
                start: Some("2015-03-10".into()),
                end: None,
            };
            match knock {
                1 => f.contested = true,
                2 => special.start = Some("2015-07-01".into()),
                3 => special.episodes = Some(k + 1),
                4 => f.map.segments[0].extras_after[0] = w + 1,
                5 => f.map.segments[0].extras_after = vec![season; k as usize],
                6 => f.map.segments[0].lead = 1,
                8 if w == 0 => special.start = Some("2015-03-20".into()),
                8 => special.start = Some("2015-03-02".into()),
                // An epilogue: its extras after the season's last episode,
                // the special dated on, after, or just before its end — or
                // too early for it.
                9..=11 => {
                    f.map.segments[0].extras_after = vec![season; k as usize];
                    special.start = Some(
                        match knock {
                            9 => "2015-06-20",
                            10 => "2015-07-15",
                            _ => "2015-06-10",
                        }
                        .into(),
                    );
                }
                _ => {}
            }
            let mut related = vec![f.clone()];
            if knock == 7 {
                let mut twin = f;
                twin.entry.id = 2;
                twin.map.segments[0].entry = 2;
                related.push(twin);
            }
            SpecialEvidence { special, related }
        })
        .boxed()
}

fn evidence() -> BoxedStrategy<SpecialEvidence> {
    prop_oneof![free_evidence(), firing_evidence()].boxed()
}

fn free_evidence() -> BoxedStrategy<SpecialEvidence> {
    (
        prop_oneof![1 => Just(None), 8 => (1u32..=3).prop_map(Some)],
        date(),
        prop::collection::vec(
            (filing(), prop_oneof![6 => Just(false), 1 => Just(true)]),
            0..=3,
        ),
    )
        .prop_map(|(episodes, start, related)| SpecialEvidence {
            special: KitsuShow {
                id: "s".into(),
                episodes,
                start,
                end: None,
            },
            related: related
                .into_iter()
                .map(|(f, contested)| RelatedFiling { contested, ..f })
                .collect(),
        })
        .boxed()
}

/// `YYYY-MM-DD` with a day that month has in 2015, by the model's own
/// table, read as `(month, day)`.
fn whole_day_model(date: Option<&str>) -> Option<(u32, u32)> {
    let d = date?;
    let b = d.as_bytes();
    if b.len() != 10 || &d[..5] != "2015-" || b[7] != b'-' {
        return None;
    }
    let (m, day): (u32, u32) = (d[5..7].parse().ok()?, d[8..].parse().ok()?);
    let length = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12)
        .contains(&m)
        .then_some(())
        .and_then(|()| (day >= 1 && day <= length[m as usize - 1]).then_some((m, day)))
}

/// The day of 2015 a whole day is, counted by the model's own table.
fn day_of_year_model(date: Option<&str>) -> Option<u32> {
    let (m, d) = whole_day_model(date)?;
    let length = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    Some(length[..m as usize - 1].iter().sum::<u32>() + d)
}

/// Whether a special that started on `special` can take the slot after
/// Kitsu's episode `after` of a season that started on `season`, as the
/// docs state it: a prologue's slot no more than two days after the
/// season's start, both whole days; a recap's only once the season is
/// settled as having started first — an earlier month, or an earlier
/// whole day in the same month.
fn dated_for_slot_model(special: Option<&str>, season: Option<&str>, after: u32) -> bool {
    if after == 0 {
        return matches!(
            (day_of_year_model(season), day_of_year_model(special)),
            (Some(a), Some(b)) if i64::from(b) - i64::from(a) <= 2
        );
    }
    match (month_model(season), month_model(special)) {
        (Some(a), Some(b)) if a != b => a < b,
        (Some(_), Some(_)) => matches!(
            (whole_day_model(season), whole_day_model(special)),
            (Some((_, a)), Some((_, b))) if a < b
        ),
        _ => false,
    }
}

/// The month of a 2015 date or month, by the model's own reading.
fn month_model(date: Option<&str>) -> Option<u32> {
    let d = date?;
    match d.len() {
        7 if &d[..5] == "2015-" => d[5..].parse().ok().filter(|m| (1..=12).contains(m)),
        10 => whole_day_model(Some(d)).map(|(m, _)| m),
        _ => None,
    }
}

/// The documented rule, written from the docs: the related entries that
/// hold the special, and the one segment that files it where exactly one
/// does.
fn special_model(ev: &SpecialEvidence) -> Option<EpisodeMap> {
    let k = ev.special.episodes.filter(|n| *n > 0)?;
    let (s, _) = whole_day_model(ev.special.start.as_deref())?;
    let held: Vec<Segment> = ev
        .related
        .iter()
        .filter(|f| !f.contested)
        .filter_map(|f| {
            let seg = match (f.map.shape, f.map.segments.as_slice()) {
                (Shape::Extras, [seg]) => seg,
                _ => return None,
            };
            let w = *seg.extras_after.first()?;
            let fits = seg.entry == f.entry.id
                && seg.kitsu_from == 1
                && seg.lead == 0
                && seg.extras_after.len() == k as usize
                && seg.extras_after.iter().all(|e| *e == w)
                && u64::from(w) + u64::from(k) <= u64::from(seg.entry_total);
            let in_run = matches!(
                (month_model(f.entry.start.as_deref()), month_model(f.entry.end.as_deref())),
                (Some(a), Some(b)) if a <= s && s <= b
            );
            // An epilogue's slot, after the season's last episode, is
            // dated by the season's end and may lie past the entry's run.
            let dated = if w == seg.kitsu_to && w > 0 {
                matches!(
                    (day_of_year_model(ev.special.start.as_deref()), day_of_year_model(f.related.end.as_deref())),
                    (Some(a), Some(b)) if i64::from(b) - i64::from(a) <= 2
                )
            } else {
                in_run
                    && dated_for_slot_model(ev.special.start.as_deref(), f.related.start.as_deref(), w)
            };
            (fits && dated).then(|| Segment {
                entry: seg.entry,
                entry_total: seg.entry_total,
                entry_finished: seg.entry_finished,
                kitsu_from: 1,
                kitsu_to: k,
                lead: w,
                extras_after: Vec::new(),
            })
        })
        .collect();
    let mut once: Vec<Segment> = Vec::new();
    for segment in held {
        if !once.contains(&segment) {
            once.push(segment);
        }
    }
    match once.as_slice() {
        [one] => Some(EpisodeMap {
            shape: Shape::SpecialExtras,
            segments: vec![one.clone()],
        }),
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// The rule is the documented one.
    #[test]
    fn a_special_is_filed_exactly_as_the_documented_rule_says(ev in evidence()) {
        prop_assert_eq!(special_extras(&ev), special_model(&ev));
    }

    /// A special filed writes every one of its episodes to its one
    /// entry, at its slot, bringing no entry up first; it completes the
    /// entry only at the entry's last episode, on an entry the tracker has
    /// finished; and each write needs the episodes before its slot on the
    /// entry, so none claims an episode the user has not watched.
    #[test]
    fn a_special_completes_its_entry_only_at_the_entry_s_last_slot(ev in evidence()) {
        let Some(map) = special_extras(&ev) else {
            return Ok(());
        };
        let k = ev.special.episodes.unwrap_or(0);
        let seg = &map.segments[0];
        for episode in 1..=k {
            match map.target(episode) {
                Target::Mapped(w) => {
                    prop_assert_eq!(w.entry, seg.entry);
                    prop_assert_eq!(w.progress, seg.lead + episode);
                    prop_assert!(w.progress >= 1 && w.progress <= seg.entry_total);
                    prop_assert_eq!(
                        w.complete,
                        w.progress == seg.entry_total && seg.entry_finished
                    );
                    prop_assert_eq!(w.needs, (seg.lead > 0).then_some(seg.lead));
                    prop_assert!(w.earlier.is_empty());
                }
                Target::Identity => prop_assert!(false, "episode {} filed nowhere", episode),
            }
        }
        prop_assert_eq!(map.target(k + 1), Target::Identity);
    }

    /// Weighed against the other rules' map: theirs where the special
    /// rule files nothing; the special's where theirs explains nothing;
    /// a conflict that stays one; theirs where both file it alike; and a
    /// conflict where both file it differently.
    #[test]
    fn the_special_rule_weighs_against_the_others_as_documented(
        ev in evidence(), existing_shape in shape(), same in any::<bool>()
    ) {
        let special = special_extras(&ev);
        let existing = match (existing_shape, &special) {
            (Shape::Identity(_), _) => EpisodeMap::identity(match existing_shape {
                Shape::Identity(why) => why,
                _ => Identity::Unexplained,
            }),
            (shape, Some(s)) if same => EpisodeMap { shape, segments: s.segments.clone() },
            (shape, _) => EpisodeMap {
                shape,
                segments: vec![Segment {
                    entry: 99,
                    entry_total: 5,
                    entry_finished: true,
                    kitsu_from: 1,
                    kitsu_to: 1,
                    lead: 4,
                    extras_after: Vec::new(),
                }],
            },
        };
        let got = with_special(existing.clone(), special.clone());
        let want = match (&existing.shape, special) {
            (_, None) => existing.clone(),
            (Shape::Identity(Identity::Conflict), Some(_)) => existing.clone(),
            (Shape::Identity(_), Some(s)) => s,
            (_, Some(s)) if s.segments == existing.segments => existing.clone(),
            (_, Some(_)) => EpisodeMap::identity(Identity::Conflict),
        };
        prop_assert_eq!(got, want);
    }
}
