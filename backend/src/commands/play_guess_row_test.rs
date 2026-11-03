//! A play asked from a guessed Continue card says whether it landed on
//! that card's own listing.

use super::*;
use crate::commands::play_native_test_provider::{stitched_recap, STITCHED, STITCHED_SLUG};
use proptest::prelude::*;

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = super::tests::state_with_proxy_origin();
    state
        .providers
        .set_stub(std::sync::Arc::new(stitched_recap()));
    state.config_path = td.path().join("config.toml");
    state.history_path = td.path().join("history");
    state
}

/// The key the stitched show plays under.
fn played_key() -> String {
    crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        STITCHED_SLUG,
    )
    .to_string()
}

fn args(guess_row: Option<&str>) -> PlayArgs {
    let mut v = serde_json::json!({
        "title": STITCHED,
        "episode": "5",
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
    });
    if let Some(row) = guess_row {
        v["guess_row"] = serde_json::json!(row);
    }
    serde_json::from_value(v).expect("args")
}

#[tokio::test]
async fn a_guessed_play_that_lands_on_its_cards_listing_says_so() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args(Some(&played_key())), |_| {})
        .await
        .expect("plays");
    assert!(resp.own_listing);
}

#[tokio::test]
async fn a_guessed_play_from_a_bare_row_with_the_listings_words_lands_on_it() {
    // anidb.app's key for the same show: its words, its own id.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args(Some("stitched-show-907")), |_| {})
        .await
        .expect("plays");
    assert!(resp.own_listing);
}

#[tokio::test]
async fn a_guessed_play_that_lands_on_another_listing_says_it_did_not() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let other = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        "another-show-4",
    )
    .to_string();
    let resp = play_with_progress(&state, &args(Some(&other)), |_| {})
        .await
        .expect("plays");
    assert!(!resp.own_listing);
}

#[tokio::test]
async fn a_play_no_guessed_card_asked_for_lands_on_no_cards_listing() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args(None), |_| {})
        .await
        .expect("plays");
    assert!(!resp.own_listing);
    // And the wire says nothing of it.
    let json = serde_json::to_value(&resp).expect("json");
    assert!(json.get("own_listing").is_none());
}

#[tokio::test]
async fn a_replayed_resolution_says_whether_it_lands_on_the_cards_listing() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = args(Some(&played_key()));
    play_resolution_cache::put(
        &state.cache_pool,
        &asked.resolution_key(),
        &CachedResolution {
            upstream_url: format!("{}/x/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: played_key(),
            show_title: STITCHED.into(),
            resolved_slot: Some(5),
            subtitles: Vec::new(),
            kitsu_id: None,
            played: Some("5".into()),
            former_episode: None,
            versions: Vec::new(),
            version: None,
            episode_unverified: false,
        },
    );
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert!(resp.own_listing);
    let elsewhere = args(Some("test-a:another-show-4"));
    let resp = play_with_progress(&state, &elsewhere, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert!(!resp.own_listing);
}

fn asked(kitsu_id: Option<&str>, guess_row: Option<&str>) -> PlayArgs {
    let mut v = serde_json::json!({ "title": "t", "episode": "1", "mode": "sub" });
    if let Some(id) = kitsu_id {
        v["kitsu_id"] = serde_json::json!(id);
    }
    if let Some(row) = guess_row {
        v["guess_row"] = serde_json::json!(row);
    }
    serde_json::from_value(v).expect("args")
}

fn key() -> impl Strategy<Value = String> {
    (
        prop::option::of(prop::sample::select(vec!["hianime", "test-a"])),
        prop::collection::vec("[a-z][a-z0-9]{0,5}", 1..4),
        1u32..50,
    )
        .prop_map(|(label, words, id)| {
            let slug = format!("{}-{id}", words.join("-"));
            label.map_or(slug.clone(), |l| format!("{l}:{slug}"))
        })
}

proptest! {
    /// The id a watch may record: the request's when no guessed card
    /// asked; when one did, the request's exactly when the played key
    /// is the card's own listing, else none — and never an id the
    /// request lacks.
    #[test]
    fn a_watch_records_the_requests_id_only_where_its_guess_lands(
        kitsu_id in prop::option::of("[1-9][0-9]{0,5}"),
        guess_row in prop::option::of(key()),
        played in key(),
        own in any::<bool>(),
    ) {
        // Half the time the card's own key is the one played.
        let played = match (&guess_row, own) {
            (Some(row), true) => row.clone(),
            _ => played,
        };
        let args = asked(kitsu_id.as_deref(), guess_row.as_deref());
        let lands = guess_row
            .as_deref()
            .is_some_and(|row| crate::scraper::provider::same_listing(row, &played));
        prop_assert_eq!(args.lands_on_guess_row(&played), lands);
        let want = match &guess_row {
            None => kitsu_id.as_deref(),
            Some(_) if lands => kitsu_id.as_deref(),
            Some(_) => None,
        };
        prop_assert_eq!(args.recordable_kitsu_id(&played), want);
    }
}

proptest! {
    /// The id a play's resolve may record: what its watch may record;
    /// else, for a play a guessed card asked for, the card's guess
    /// exactly when the played key is the card's own listing. A guess
    /// with no card's row behind it, or one that landed elsewhere, is
    /// never recorded.
    #[test]
    fn a_resolve_records_a_guess_only_where_it_lands(
        kitsu_id in prop::option::of("[1-9][0-9]{0,5}"),
        guess_id in prop::option::of("[1-9][0-9]{0,5}"),
        guess_row in prop::option::of(key()),
        played in key(),
        own in any::<bool>(),
    ) {
        let played = match (&guess_row, own) {
            (Some(row), true) => row.clone(),
            _ => played,
        };
        let mut v = serde_json::json!({ "title": "t", "episode": "1", "mode": "sub" });
        if let Some(id) = &kitsu_id {
            v["kitsu_id"] = serde_json::json!(id);
        }
        if let Some(id) = &guess_id {
            v["guess_id"] = serde_json::json!(id);
        }
        if let Some(row) = &guess_row {
            v["guess_row"] = serde_json::json!(row);
        }
        let args: PlayArgs = serde_json::from_value(v).expect("args");
        let lands = args.lands_on_guess_row(&played);
        let want = match &guess_row {
            None => kitsu_id.as_deref(),
            Some(_) if lands => kitsu_id.as_deref().or(guess_id.as_deref()),
            Some(_) => None,
        };
        prop_assert_eq!(args.resolve_kitsu_id(&played), want);
    }
}

/// The stitched show's play from the card whose row is the played key,
/// linked to 4013, naming `guess_id` as its guess.
async fn linked_play(guess_id: &str) -> (bool, Option<String>) {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    crate::history::write_atomic(
        &state.history_path,
        &[crate::history::HistoryEntry {
            ep_no: "4".into(),
            id: played_key(),
            title: STITCHED.into(),
            watched_at: None,
            kitsu_id: Some("4013".into()),
            own_number: true,
        }],
    )
    .expect("history");
    let mut asked = args(Some(&played_key()));
    asked.guess_id = Some(guess_id.into());
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays");
    let recorded = crate::history::read_all(&state.history_path)
        .expect("rows")
        .into_iter()
        .find(|r| r.id == played_key())
        .and_then(|r| r.kitsu_id);
    (resp.own_listing, recorded)
}

#[tokio::test]
async fn a_guessed_play_on_a_linked_line_does_not_stand_for_it() {
    // The guess (7158) is not the show the line records: the play is not
    // the line's, so the renderer records and mirrors nothing of it.
    let (own, recorded) = linked_play("7158").await;
    assert!(!own);
    assert_eq!(recorded.as_deref(), Some("4013"));
}

#[tokio::test]
async fn a_play_naming_the_link_stands_for_the_linked_line() {
    let (own, recorded) = linked_play("4013").await;
    assert!(own);
    assert_eq!(recorded.as_deref(), Some("4013"));
}
