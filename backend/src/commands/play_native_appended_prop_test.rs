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

/// A title for episode `n` of the kind a listing or Kitsu gives: none,
/// one that reduces to nothing, only the number, the episode's own
/// name in one casing or another, or another name.
fn some_title(n: u32) -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("—".to_string())),
        Just(Some(format!("Episode {n}"))),
        Just(Some(format!("Story {n}"))),
        Just(Some(format!("story {n}"))),
        Just(Some("Another Name".to_string())),
    ]
}

/// Kitsu's rows for the episodes in `numbers`, each titled and dated as
/// it may be.
fn kitsu_rows(numbers: Vec<u32>) -> impl Strategy<Value = Vec<TailEpisode>> {
    numbers
        .into_iter()
        .map(|n| {
            (some_title(n), any::<bool>()).prop_map(move |(title, dated)| TailEpisode {
                number: n,
                title,
                airdate: dated.then(|| "2020-01-01".to_string()),
            })
        })
        .collect::<Vec<_>>()
}

/// A count, a listing of it with the count's last episodes named as
/// they may be, the rows of Kitsu's read so far, and rows still to
/// read: Kitsu's for any of the count's last episodes, read or not.
fn alignment_case(
) -> impl Strategy<Value = (u32, Vec<EpisodeRef>, Vec<TailEpisode>, Vec<TailEpisode>)> {
    (1u32..40).prop_flat_map(|count| {
        let first = count.saturating_sub(ALIGNED - 1).max(1);
        let last: Vec<u32> = (first..=count).collect();
        let names = last.iter().map(|&n| some_title(n)).collect::<Vec<_>>();
        let read =
            proptest::sample::subsequence(last.clone(), 0..=last.len()).prop_flat_map(kitsu_rows);
        let later =
            proptest::sample::subsequence(last.clone(), 0..=last.len()).prop_flat_map(kitsu_rows);
        (Just(count), names, read, later).prop_map(move |(count, names, read, later)| {
            let mut rows = listing(count, 1);
            for (row, name) in rows[first as usize - 1..].iter_mut().zip(names) {
                row.title = name;
            }
            (count, rows, read, later)
        })
    })
}

fn read_so_far(episodes: Vec<TailEpisode>) -> EntryTail {
    EntryTail {
        episodes,
        ..EntryTail::default()
    }
}

proptest! {
    // The cases that decide these are a few named shapes of three
    // episodes; enough runs to meet each of them.
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// What the misalignment check says is settled stays settled:
    /// whatever Kitsu rows are read after, the listing does not line
    /// up with Kitsu's last episodes.
    #[test]
    fn a_misalignment_settled_so_far_holds_whatever_is_read_after(
        (count, rows, read, later) in alignment_case()
    ) {
        let so_far = read_so_far(read.clone());
        if misaligned_so_far(&rows, count, &so_far) {
            let mut all = read;
            all.extend(later);
            prop_assert!(!aligned(&rows, count, &read_so_far(all)));
        }
    }

    /// Once every one of the count's last episodes is read, the check
    /// says exactly what the rule does: nothing still to read could
    /// change it.
    #[test]
    fn with_every_last_episode_read_the_check_is_the_rule(
        (count, rows, read, later) in alignment_case()
    ) {
        let mut all = read;
        all.extend(later);
        let first = count.saturating_sub(ALIGNED - 1).max(1);
        let tail = read_so_far(all);
        if (first..=count).all(|n| tail.episodes.iter().any(|e| e.number == n)) {
            prop_assert_eq!(misaligned_so_far(&rows, count, &tail), !aligned(&rows, count, &tail));
        }
    }

    /// The check settles exactly when nothing still to read could line
    /// the listing up: with every unread last episode read the best way
    /// it could be, dated and titled as the listing names it, the rule
    /// says what the check said before it.
    #[test]
    fn the_check_settles_exactly_when_nothing_unread_could_line_up(
        (count, rows, read, _later) in alignment_case()
    ) {
        let first = count.saturating_sub(ALIGNED - 1).max(1);
        let mut best = read.clone();
        for n in first..=count {
            if !read.iter().any(|e| e.number == n) {
                best.push(TailEpisode {
                    number: n,
                    title: rows[n as usize - 1].title.clone(),
                    airdate: Some("2020-01-01".into()),
                });
            }
        }
        prop_assert_eq!(
            misaligned_so_far(&rows, count, &read_so_far(read)),
            !aligned(&rows, count, &read_so_far(best))
        );
    }

    /// A row past the count carrying the listing's name for one of the
    /// count's last episodes is refused whatever Kitsu titles them:
    /// either the listing does not line up, or the row is named as one
    /// of the show's own episodes.
    #[test]
    fn a_row_past_the_count_named_as_a_last_episode_is_refused_whatever_kitsu_says(
        count in 1u32..40,
        past in 1u32..=MAX_APPENDED,
        renamed in 0u32..MAX_APPENDED,
        named_as in 0u32..ALIGNED,
        titles in proptest::collection::vec(prop_oneof![
            Just(None),
            Just(Some("Episode".to_string())),
            Just(Some("story".to_string())),
            Just(Some("Another Name".to_string())),
        ], ALIGNED as usize),
    ) {
        let mut rows = listing(count, past);
        let first = count.saturating_sub(ALIGNED - 1).max(1);
        let n = (first + named_as).min(count);
        let at = (count + renamed % past) as usize;
        rows[at].title = rows[n as usize - 1].title.clone();
        let mut tail = conclusive(count, past);
        for (i, title) in titles.into_iter().enumerate() {
            let m = first + u32::try_from(i).expect("small");
            if let Some(row) = tail.episodes.iter_mut().find(|e| e.number == m) {
                row.title = title.map(|t| if t == "story" { format!("story {m}") } else { t });
            }
        }
        prop_assert!(past_named_as_a_last_episode(&rows, count, &rows[count as usize..]));
        prop_assert_eq!(appended_extras(&rows, count, &tail), None);
    }
}

proptest! {
    /// A row is past an unread count exactly when a count went unread
    /// and the row's slot lies beyond it: no row is, where every count
    /// was read, and the count's own last slot never is.
    #[test]
    fn a_row_is_past_an_unread_count_only_beyond_it(
        unread_past in proptest::option::of(any::<u32>()),
        slot in any::<u32>(),
    ) {
        let got = past_an_unread_count(unread_past, slot);
        prop_assert_eq!(got, unread_past.is_some_and(|c| u64::from(slot) > u64::from(c)));
        prop_assert!(!past_an_unread_count(None, slot));
        if let Some(count) = unread_past {
            prop_assert!(!past_an_unread_count(Some(count), count));
            prop_assert_eq!(past_an_unread_count(Some(count), count.saturating_add(1)), count < u32::MAX);
        }
    }
}
