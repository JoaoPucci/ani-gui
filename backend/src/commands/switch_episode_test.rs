//! The episode "Switch to it now" opens on the show picked for a card.

use super::*;
use proptest::prelude::*;

#[test]
fn a_pick_with_further_progress_opens_where_the_viewer_left_it() {
    // Stardust Crusaders' own row is at episode 6, watched through:
    // Continue goes on to 7, past the card's episode 2.
    assert_eq!(switch_episode(2.0, Some(6.0), Some(48), &[]), Some(7.0));
    // Left part-way at 6: Continue plays 6 again.
    assert_eq!(switch_episode(2.0, Some(6.0), Some(48), &[6.0]), Some(6.0));
}

#[test]
fn a_pick_with_less_or_equal_progress_takes_the_cards_episode() {
    assert_eq!(switch_episode(5.0, Some(3.0), Some(48), &[]), None);
    // Continue would offer 5, the episode playing: nothing moves back.
    assert_eq!(switch_episode(5.0, Some(4.0), Some(48), &[]), None);
    assert_eq!(switch_episode(5.0, Some(5.0), Some(48), &[5.0]), None);
}

#[test]
fn a_pick_with_no_progress_of_its_own_takes_the_cards_episode() {
    assert_eq!(switch_episode(2.0, None, Some(48), &[]), None);
}

#[test]
fn a_pick_at_its_last_episode_opens_that_episode_as_continue_does() {
    // Continue replays the last episode of a show watched to its end.
    assert_eq!(switch_episode(2.0, Some(12.0), Some(12), &[]), Some(12.0));
    // With no count known, Continue goes on.
    assert_eq!(switch_episode(2.0, Some(12.0), None, &[]), Some(13.0));
}

#[test]
fn an_extra_goes_on_to_the_whole_episode_above_it() {
    assert_eq!(switch_episode(2.0, Some(7.5), Some(24), &[]), Some(8.0));
    assert_eq!(switch_episode(2.0, Some(7.5), Some(24), &[7.5]), Some(7.5));
}

#[test]
fn a_row_episode_reads_as_the_history_reads_it() {
    assert_eq!(row_episode("6"), Some(6.0));
    assert_eq!(row_episode("7.5"), Some(7.5));
    assert_eq!(row_episode("1-12"), Some(1.0));
    assert_eq!(row_episode(""), None);
    assert_eq!(row_episode("0"), None);
    assert_eq!(row_episode("x"), None);
    // As the renderer's `parseHistoryEpisode` reads them.
    assert_eq!(row_episode(".5"), None);
    assert_eq!(row_episode("7.5.3"), Some(7.5));
    assert_eq!(row_episode("  12 extra"), Some(12.0));
    assert_eq!(row_episode("6."), Some(6.0));
}

/// The renderer's `parseHistoryEpisode`, `/^\s*(\d+(?:\.\d+)?)/`, by
/// hand: the leading digits, then a dot and digits when both follow.
fn parse_history_episode(raw: &str) -> Option<f64> {
    let t = raw.trim_start();
    let whole: String = t.chars().take_while(char::is_ascii_digit).collect();
    if whole.is_empty() {
        return None;
    }
    let rest = &t[whole.len()..];
    let frac: String = rest
        .strip_prefix('.')
        .map(|r| r.chars().take_while(char::is_ascii_digit).collect())
        .unwrap_or_default();
    let text = if frac.is_empty() {
        whole
    } else {
        format!("{whole}.{frac}")
    };
    text.parse::<f64>().ok().filter(|n| *n > 0.0)
}

#[test]
fn the_answer_and_request_speak_the_renderers_wire() {
    let args: SwitchArgs = serde_json::from_value(serde_json::json!({
        "pick": "8063", "card": "hianime:jojo-diu-1388", "episode": 2
    }))
    .unwrap();
    assert_eq!(args.episode_count, None);
    assert!(args.part_way.is_empty());
    assert_eq!(
        serde_json::to_value(SwitchEpisode { episode: Some(7.0) }).unwrap(),
        serde_json::json!({ "episode": 7.0 })
    );
    assert_eq!(
        serde_json::to_value(SwitchEpisode { episode: None }).unwrap(),
        serde_json::json!({ "episode": null })
    );
}

proptest! {
    /// A row's episode reads exactly as the renderer reads it.
    #[test]
    fn a_row_episode_reads_as_the_renderer_reads_it(
        raw in "[ ]{0,2}[0-9]{0,3}(\\.[0-9]{0,2}){0,2}[-a-z ]{0,3}",
    ) {
        prop_assert_eq!(row_episode(&raw), parse_history_episode(&raw));
    }

    /// The rule over fractions and any cap: never behind the card's
    /// episode, never behind the pick's own, and exactly Continue's.
    #[test]
    fn a_switch_opens_continues_episode_exactly_when_it_is_past_the_cards(
        card in 1u32..120,
        own in proptest::option::of(1u32..120),
        cap in proptest::option::of(0u32..60),
        part in any::<bool>(),
    ) {
        let card = f64::from(card) / 2.0;
        let own = own.map(|o| f64::from(o) / 2.0);
        let part_way: Vec<f64> = own.filter(|_| part).into_iter().collect();
        let continues = own.map(|o| {
            let next = if o.fract() == 0.0 { o + 1.0 } else { o.ceil() };
            if part || cap.is_some_and(|c| next > f64::from(c)) { o } else { next }
        });
        let opened = switch_episode(card, own, cap, &part_way);
        prop_assert_eq!(opened, continues.filter(|c| *c > card));
    }

    /// A switch never opens the pick behind the episode playing, nor
    /// behind where Continue would take the pick; it opens the pick's
    /// own episode exactly when that is past the card's.
    #[test]
    fn a_switch_never_takes_the_pick_back(
        card in 1u32..60,
        own in proptest::option::of(1u32..60),
        cap in proptest::option::of(1u32..60),
        part in any::<bool>(),
    ) {
        let card = f64::from(card);
        let own = own.map(f64::from);
        let part_way: Vec<f64> = own.filter(|_| part).into_iter().collect();
        let cap = cap.map(|c| own.map_or(c, |o| c.max(o as u32)));
        let opened = switch_episode(card, own, cap, &part_way);
        match (own, opened) {
            (None, o) => prop_assert_eq!(o, None),
            (Some(o), Some(e)) => {
                prop_assert!(e > card);
                prop_assert!(e >= o);
                prop_assert!(e == o || e == o + 1.0);
            }
            (Some(o), None) => {
                let resume = if part || cap.is_some_and(|c| o + 1.0 > f64::from(c)) { o } else { o + 1.0 };
                prop_assert!(resume <= card);
            }
        }
    }
}
