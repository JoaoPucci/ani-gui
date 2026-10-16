//! Properties of the appended-extras rule: whatever the listing and
//! the evidence, no whole episode up to the count changes row, and
//! what moves is exactly the rows past the count, as extras.

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

/// A name from a small alphabet, so listings and Kitsu's rows collide
/// often: equal names, generic ones, missing ones.
fn name() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("Episode 13".to_string())),
        "[A-C]{1,3}".prop_map(Some),
        "[a-c ]{1,4}".prop_map(Some),
    ]
}

/// A listing of up to 30 rows numbered by slot, now and then a row
/// carrying a tag.
fn listing() -> impl Strategy<Value = Vec<EpisodeRef>> {
    proptest::collection::vec(
        (name(), proptest::option::weighted(0.05, "[0-9]{1,2}\\.5")),
        0..30,
    )
    .prop_map(|rows| {
        rows.into_iter()
            .zip(1u32..)
            .map(|((title, tag), n)| EpisodeRef {
                id: u64::from(n) * 7,
                number: n,
                number2: tag,
                title,
            })
            .collect()
    })
}

/// Evidence that is sometimes conclusive: built from the listing's own
/// titles up to `count` and then perturbed field by field.
fn tail_for(rows: &[EpisodeRef], count: u32) -> impl Strategy<Value = EntryTail> {
    let own: Vec<TailEpisode> = rows
        .iter()
        .filter(|e| e.number <= count)
        .map(|e| TailEpisode {
            number: e.number,
            title: e.title.clone(),
            airdate: Some("2020-01-01".into()),
        })
        .collect();
    (
        any::<bool>(),
        prop_oneof![Just(Some(count)), proptest::option::of(0u32..40)],
        prop_oneof![Just(Some(count)), proptest::option::of(0u32..40)],
        any::<bool>(),
        proptest::collection::vec((0u32..40, name(), any::<bool>()), 0..3),
    )
        .prop_map(move |(finished, kitsu, anilist, past, noise)| {
            let mut episodes = own.clone();
            episodes.extend(noise.into_iter().map(|(number, title, dated)| TailEpisode {
                number,
                title,
                airdate: dated.then(|| "2020-01-01".to_string()),
            }));
            EntryTail {
                finished,
                count: kitsu,
                anilist_count: anilist,
                episodes,
                read_past_count: past,
                related: Vec::new(),
                related_complete: false,
            }
        })
}

fn case() -> impl Strategy<Value = (Vec<EpisodeRef>, u32, EntryTail)> {
    listing().prop_flat_map(|rows| {
        let len = u32::try_from(rows.len()).unwrap_or(0);
        (Just(rows), 0..=len + 2).prop_flat_map(|(rows, count)| {
            let tail = tail_for(&rows, count);
            (Just(rows), Just(count), tail)
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Whatever the evidence, every whole episode up to the count
    /// plays the row it played before — none shifted, skipped or
    /// doubled — and nothing past the count is left a whole episode.
    #[test]
    fn no_whole_episode_up_to_the_count_changes_row((rows, count, tail) in case()) {
        let Some(fitted) = appended_extras(&rows, count, &tail) else {
            return Ok(());
        };
        prop_assert_eq!(fitted.len(), rows.len());
        for n in 1..=count {
            prop_assert_eq!(row_for(&fitted, n), row_for(&rows, n), "episode {}", n);
        }
        prop_assert_eq!(kitsu_episode_cap(&fitted), Some(count));
        prop_assert_eq!(numbering_offset(&fitted), 0);
        for (before, after) in rows.iter().zip(&fitted) {
            prop_assert_eq!(
                (before.id, before.number, &before.title),
                (after.id, after.number, &after.title)
            );
        }
        let past: Vec<String> = (1..=fitted.len() - count as usize)
            .map(|i| format!("{count}.{i}"))
            .collect();
        prop_assert_eq!(extra_episode_tags(&fitted), past);
    }

    /// The rule moves nothing on a listing the shape check turns away,
    /// and nothing without every piece of evidence it names.
    #[test]
    fn short_evidence_moves_nothing((rows, count, tail) in case()) {
        let fitted = appended_extras(&rows, count, &tail);
        let asks = appended_count(&rows, Some(count)) == Some(count);
        let proven = tail.finished
            && tail.count == Some(count)
            && tail.anilist_count == Some(count)
            && tail.read_past_count
            && tail.episodes.iter().all(|e| e.number <= count);
        if !(asks && proven) {
            prop_assert_eq!(fitted, None);
        }
    }

    /// Conclusive evidence — Kitsu and AniList end the show at the
    /// count, Kitsu's rows up to it are dated and named as the
    /// listing names them, and each row past it has a name of its
    /// own — reads exactly the rows past the count as extras, in order.
    #[test]
    fn conclusive_evidence_reads_the_rows_past_the_count_as_extras(
        count in 1u32..30,
        past in 1u32..=MAX_APPENDED,
    ) {
        let rows: Vec<EpisodeRef> = (1..=count + past)
            .map(|n| EpisodeRef {
                id: u64::from(n),
                number: n,
                number2: None,
                title: Some(format!("Story {n}")),
            })
            .collect();
        let tail = EntryTail {
            finished: true,
            count: Some(count),
            anilist_count: Some(count),
            episodes: (count.saturating_sub(4)..=count)
                .filter(|n| *n > 0)
                .map(|n| TailEpisode {
                    number: n,
                    title: Some(format!("story {n}")),
                    airdate: Some("2020-01-01".into()),
                })
                .collect(),
            read_past_count: true,
            related: Vec::new(),
            related_complete: false,
        };
        prop_assert_eq!(appended_count(&rows, Some(count)), Some(count));
        let fitted = appended_extras(&rows, count, &tail);
        prop_assert!(fitted.is_some());
        let want: Vec<String> = (1..=past).map(|i| format!("{count}.{i}")).collect();
        prop_assert_eq!(extra_episode_tags(&fitted.expect("fitted")), want);
    }

    /// Every tag the rule writes reads back as the number it is, so
    /// a request carrying it round-trips through any reader that
    /// parses it.
    #[test]
    fn every_tag_reads_back_as_itself((rows, count, tail) in case()) {
        if let Some(fitted) = appended_extras(&rows, count, &tail) {
            for tag in extra_episode_tags(&fitted) {
                let n: f64 = tag.parse().expect("a number");
                prop_assert_eq!(n.to_string(), tag);
            }
        }
    }
}
