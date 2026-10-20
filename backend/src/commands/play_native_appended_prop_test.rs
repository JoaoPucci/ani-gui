//! Properties of the appended-extras rule: whatever the listing and
//! the evidence, no whole episode up to the count changes row, and
//! what moves is exactly the rows past the count, as extras.
//!
//! The cases start from conclusive evidence — a listing of distinct
//! titles running a few rows past the count, Kitsu and AniList ending
//! the show at the count, Kitsu's rows named as the listing names
//! them, an OVA of the right length, a finished TV sequel whose first
//! episodes Kitsu titles — and break at most one piece of it, so both
//! readings are reached in every run.

use super::*;
use crate::commands::play_native_numbering::{
    extra_episode_tags, kitsu_episode_cap, numbering_offset,
};
use proptest::prelude::*;

/// The row a listing plays for whole episode `n`: the row whose
/// display number is `n` — its tag where it has one, its slot where it
/// has none — as the resolver reads it.
fn row_for(rows: &[EpisodeRef], n: u32) -> Option<&EpisodeRef> {
    rows.iter().find(|e| match e.number2.as_deref() {
        Some(tag) => tag.parse::<u32>() == Ok(n),
        None => e.number == n,
    })
}

/// A listing of `count + past` rows, each named for its slot.
fn listing(count: u32, past: u32) -> Vec<EpisodeRef> {
    (1..=count + past)
        .map(|n| EpisodeRef {
            id: u64::from(n) * 7,
            number: n,
            number2: None,
            title: Some(format!("Story {n}")),
            former_tag: None,
        })
        .collect()
}

/// Evidence that proves the `past` rows after `count` are extras.
fn conclusive(count: u32, past: u32) -> EntryTail {
    EntryTail {
        finished: true,
        count: Some(count),
        anilist_count: Some(count),
        episodes: (1..=count)
            .map(|n| TailEpisode {
                number: n,
                title: Some(format!("story {n}")),
                airdate: Some("2020-01-01".into()),
            })
            .collect(),
        read_past_count: true,
        related: vec![
            Related {
                role: "side_story".into(),
                anime: true,
                subtype: Some("OVA".into()),
                status: Some("finished".into()),
                episode_count: Some(past),
                first_episodes: None,
            },
            Related {
                role: "sequel".into(),
                anime: true,
                subtype: Some("TV".into()),
                status: Some("finished".into()),
                episode_count: Some(past + 2),
                first_episodes: Some(
                    (1..=past + 2)
                        .map(|n| SequelEpisode {
                            number: n,
                            title: Some(format!("Sequel {n}")),
                        })
                        .collect(),
                ),
            },
        ],
        related_complete: true,
    }
}

/// One way of breaking the evidence or the listing, or none.
#[derive(Debug, Clone)]
enum Break {
    None,
    Unfinished,
    KitsuCount(u32),
    AnilistCount(Option<u32>),
    NotReadPast,
    KitsuPast,
    KitsuRowGone(u32),
    Undated(u32),
    Renamed(u32),
    Untitled(u32),
    Generic(u32),
    Tagged(u32),
    OvaLength(u32),
    SequelAiring,
    SequelUnread,
    SequelUntitled(u32),
    SequelMissing(u32),
    SequelNamed(u32),
    OtherTvAiring,
    OtherTvUntitled,
    RelationsInPart,
}

fn a_break(count: u32, past: u32) -> impl Strategy<Value = Break> {
    let row = 1..=count + past;
    prop_oneof![
        3 => Just(Break::None),
        1 => Just(Break::Unfinished),
        1 => (0u32..40).prop_map(Break::KitsuCount),
        1 => proptest::option::of(0u32..40).prop_map(Break::AnilistCount),
        1 => Just(Break::NotReadPast),
        1 => Just(Break::KitsuPast),
        1 => (1..=count).prop_map(Break::KitsuRowGone),
        1 => (1..=count).prop_map(Break::Undated),
        1 => row.clone().prop_map(Break::Renamed),
        1 => row.clone().prop_map(Break::Untitled),
        1 => row.clone().prop_map(Break::Generic),
        1 => row.prop_map(Break::Tagged),
        1 => (0u32..12).prop_map(Break::OvaLength),
        1 => Just(Break::SequelAiring),
        1 => Just(Break::SequelUnread),
        1 => (1..=past + 2).prop_map(Break::SequelUntitled),
        1 => (1..=past + 2).prop_map(Break::SequelMissing),
        1 => (1..=past).prop_map(Break::SequelNamed),
        1 => Just(Break::OtherTvAiring),
        1 => Just(Break::OtherTvUntitled),
        1 => Just(Break::RelationsInPart),
    ]
}

/// The listing and evidence `b` leaves.
fn broken(count: u32, past: u32, b: &Break) -> (Vec<EpisodeRef>, EntryTail) {
    let mut rows = listing(count, past);
    let mut tail = conclusive(count, past);
    let at = |n: u32| usize::try_from(n - 1).expect("an index");
    match b {
        Break::None => {}
        Break::Unfinished => tail.finished = false,
        Break::KitsuCount(c) => tail.count = Some(*c),
        Break::AnilistCount(c) => tail.anilist_count = *c,
        Break::NotReadPast => tail.read_past_count = false,
        Break::KitsuPast => tail.episodes.push(TailEpisode {
            number: count + 1,
            title: Some("More".into()),
            airdate: Some("2020-01-01".into()),
        }),
        Break::KitsuRowGone(n) => tail.episodes.retain(|e| e.number != *n),
        Break::Undated(n) => tail.episodes[at(*n)].airdate = None,
        Break::Renamed(n) => rows[at(*n)].title = Some("Somewhere Else".into()),
        Break::Untitled(n) => rows[at(*n)].title = None,
        Break::Generic(n) => rows[at(*n)].title = Some(format!("Episode {n}")),
        Break::Tagged(n) => rows[at(*n)].number2 = Some(format!("{n}.5")),
        Break::OvaLength(l) => tail.related[0].episode_count = Some(*l),
        Break::SequelAiring => tail.related.push(Related {
            role: "sequel".into(),
            anime: true,
            subtype: Some("TV".into()),
            status: Some("current".into()),
            episode_count: None,
            first_episodes: Some(Vec::new()),
        }),
        Break::SequelUnread => tail.related[1].first_episodes = None,
        Break::SequelUntitled(n) => sequel_episode(&mut tail, *n).title = None,
        Break::SequelMissing(n) => {
            let page = tail.related[1]
                .first_episodes
                .as_mut()
                .expect("the sequel's page");
            page.retain(|e| e.number != *n);
        }
        Break::SequelNamed(i) => {
            sequel_episode(&mut tail, *i).title = rows[at(count + i)].title.clone();
        }
        Break::OtherTvAiring => tail.related.push(other_tv("current", Some("Other 1"))),
        Break::OtherTvUntitled => tail.related.push(other_tv("finished", None)),
        Break::RelationsInPart => tail.related_complete = false,
    }
    (rows, tail)
}

/// A TV entry Kitsu relates as `other`, with `status`, its first
/// episode titled `first` and the rest titled.
fn other_tv(status: &str, first: Option<&str>) -> Related {
    Related {
        role: "other".into(),
        anime: true,
        subtype: Some("TV".into()),
        status: Some(status.into()),
        episode_count: Some(12),
        first_episodes: Some(
            (1..=12)
                .map(|n| SequelEpisode {
                    number: n,
                    title: if n == 1 {
                        first.map(str::to_string)
                    } else {
                        Some(format!("Other {n}"))
                    },
                })
                .collect(),
        ),
    }
}

/// The sequel's episode `n` in the conclusive evidence.
fn sequel_episode(tail: &mut EntryTail, n: u32) -> &mut SequelEpisode {
    tail.related[1]
        .first_episodes
        .as_mut()
        .expect("the sequel's page")
        .iter_mut()
        .find(|e| e.number == n)
        .expect("an episode of the sequel")
}

/// Whether `b` leaves the evidence conclusive: the breaks that touch
/// nothing the rule reads — a row before the last three, a row past
/// the count renamed to another name of its own, a sequel episode past
/// the first few — or change a value to what it was.
fn still_conclusive(count: u32, past: u32, b: &Break) -> bool {
    let first = count.saturating_sub(ALIGNED - 1).max(1);
    match b {
        Break::None => true,
        Break::KitsuCount(c) => *c == count,
        Break::AnilistCount(c) => *c == Some(count),
        Break::KitsuRowGone(n) | Break::Undated(n) => *n < first,
        Break::Renamed(n) => *n < first || *n > count,
        Break::Untitled(n) | Break::Generic(n) => *n < first,
        Break::OvaLength(l) => *l == past,
        // Only the sequel's first episodes, as many as the rows past
        // the count, are read.
        Break::SequelUntitled(n) | Break::SequelMissing(n) => *n > past,
        _ => false,
    }
}

fn case() -> impl Strategy<Value = (u32, u32, Break)> {
    (1u32..30, 1u32..=MAX_APPENDED)
        .prop_flat_map(|(count, past)| (Just(count), Just(past), a_break(count, past)))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Conclusive evidence reads exactly the rows past the count as
    /// extras, in order; breaking any piece of it leaves the listing
    /// as it was.
    #[test]
    fn the_rows_past_the_count_are_extras_exactly_when_the_evidence_holds(
        (count, past, b) in case()
    ) {
        let (rows, tail) = broken(count, past, &b);
        let fitted = appended_extras(&rows, count, &tail);
        if still_conclusive(count, past, &b) {
            let want: Vec<String> = (1..=past).map(|i| format!("{count}.{i}")).collect();
            let fitted = fitted.expect("conclusive evidence reads the extras");
            prop_assert_eq!(extra_episode_tags(&fitted), want);
        } else {
            prop_assert_eq!(fitted, None);
        }
    }

    /// Whenever the rule reads extras, every whole episode up to the
    /// count plays the row it played before — none shifted, skipped or
    /// doubled — nothing past the count is left a whole episode, and
    /// each row is the row it was but for its tag.
    #[test]
    fn no_whole_episode_up_to_the_count_changes_row((count, past, b) in case()) {
        let (rows, tail) = broken(count, past, &b);
        let Some(fitted) = appended_extras(&rows, count, &tail) else {
            return Ok(());
        };
        prop_assert_eq!(fitted.len(), rows.len());
        for n in 1..=count {
            prop_assert_eq!(row_for(&fitted, n), row_for(&rows, n), "episode {}", n);
        }
        prop_assert_eq!(row_for(&fitted, count + 1), None);
        prop_assert_eq!(kitsu_episode_cap(&fitted), Some(count));
        prop_assert_eq!(numbering_offset(&fitted), 0);
        for (before, after) in rows.iter().zip(&fitted) {
            prop_assert_eq!(
                (before.id, before.number, &before.title),
                (after.id, after.number, &after.title)
            );
        }
    }

    /// Every tag the rule writes reads back as the number it is, so a
    /// request carrying it round-trips through any reader that parses
    /// it.
    #[test]
    fn every_tag_reads_back_as_itself(count in 1u32..1000, past in 1u32..=MAX_APPENDED) {
        let fitted = appended_extras(&listing(count, past), count, &conclusive(count, past))
            .expect("conclusive");
        for tag in extra_episode_tags(&fitted) {
            let n: f64 = tag.parse().expect("a number");
            prop_assert_eq!(n.to_string(), tag);
        }
    }
}
