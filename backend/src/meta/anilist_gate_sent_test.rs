//! What the gate logs at the app's default level for each request it
//! sends: one line naming what the request is for, its class and how
//! many shows it carries — and never a show's id. Every transport and
//! every fetcher behind it is driven once against a mock AniList, so a
//! fetcher that sent unlabelled, or under another's label, shows here.

use std::future::Future;
use std::sync::{Arc, Mutex};

use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::account::provider::{EntryUpdate, ProviderMediaId, Tokens, UserListProvider};
use crate::meta::anilist_batch::Want;
use crate::meta::anilist_gate::in_background;
use crate::meta::anilist_media::{banner_for_media, MediaRef};
use crate::meta::anilist_user::AniListProvider;

/// A body every parser reads without a media, so no fetcher asks again.
const EMPTY: &str =
    r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[]},"Media":null}}"#;

/// An id no other number in a line could be mistaken for.
const ID: u32 = 987_654;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

/// The gate's sent lines, under the app's default filter, while `f`
/// runs on a fresh runtime.
pub(crate) fn sent_lines_of<F: Future>(f: impl FnOnce() -> F) -> Vec<String> {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            crate::DEFAULT_LOG_FILTER,
        ))
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    // See `anilist_gate_test.rs`: a second dispatcher kept alive makes
    // every callsite registration ask this test's subscriber too.
    let _second = tracing::Dispatch::new(tracing_subscriber::registry());
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        runtime.block_on(f());
    });
    let out =
        String::from_utf8_lossy(&captured.0.lock().unwrap_or_else(|e| e.into_inner())).into_owned();
    out.lines()
        .filter(|l| l.contains("anilist request sent"))
        .map(str::to_owned)
        .collect()
}

/// The gate's sent lines while `f` runs against a fresh mock AniList
/// that answers every request with no media.
fn sent_lines<F: Future>(f: impl FnOnce(String) -> F) -> Vec<String> {
    sent_lines_of(|| async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(EMPTY))
            .mount(&server)
            .await;
        f(server.uri()).await;
    })
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// The one line `f` sent, checked for its level and for carrying no id.
fn one_line<F: Future>(f: impl FnOnce(String) -> F) -> String {
    let lines = sent_lines(f);
    assert_eq!(lines.len(), 1, "{lines:#?}");
    let line = lines.into_iter().next().unwrap_or_default();
    assert!(line.contains(" INFO "), "{line}");
    assert!(!line.contains(&ID.to_string()), "an id in the log: {line}");
    line
}

fn assert_labelled(line: &str, purpose: &str, shows: Option<usize>) {
    assert!(line.contains(&format!("purpose={purpose} ")), "{line}");
    match shows {
        Some(n) => assert!(line.contains(&format!("shows={n} ")), "{line}"),
        None => assert!(!line.contains("shows="), "{line}"),
    }
}

#[test]
fn the_trending_feed_is_labelled_and_carries_no_show_count() {
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist::trending(&client(), 25, Some(&uri)).await;
    });
    assert_labelled(&line, "trending", None);
    assert!(line.contains("class=Interactive"), "{line}");
}

#[test]
fn a_background_send_says_so() {
    let line = one_line(|uri| async move {
        let _ = in_background(banner_for_media(&client(), MediaRef::Mal(ID), Some(&uri))).await;
    });
    assert_labelled(&line, "banner", Some(1));
    assert!(line.contains("class=Background"), "{line}");
}

#[test]
fn a_shows_episode_stills_are_labelled() {
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist_streaming_eps::streaming_listing_for_media(
            &client(),
            MediaRef::Mal(ID),
            Some(&uri),
        )
        .await;
    });
    assert_labelled(&line, "stills", Some(1));
}

#[test]
fn a_lane_page_counts_the_shows_it_carries() {
    let wants: Vec<Want> = (0..3)
        .map(|i| Want {
            mal: Some(ID + i),
            anilist: None,
        })
        .collect();
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist_streaming_eps::streaming_listings_batch(
            &client(),
            &wants,
            Some(&uri),
        )
        .await;
    });
    assert_labelled(&line, "stills", Some(3));
    let line = one_line(|uri| async move {
        let wants: Vec<Want> = (0..2)
            .map(|i| Want {
                mal: Some(ID + i),
                anilist: None,
            })
            .collect();
        let _ = crate::meta::anilist_media::banners_batch(&client(), &wants, Some(&uri)).await;
    });
    assert_labelled(&line, "banner", Some(2));
}

#[test]
fn airing_lookups_are_labelled_singly_and_by_page() {
    let line = one_line(|uri| async move {
        let _ =
            crate::meta::anilist_airing::airing_status(&client(), Some(ID), None, Some(&uri)).await;
    });
    assert_labelled(&line, "airing", Some(1));
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist_airing::airing_status_batch(
            &client(),
            &[ID, ID + 1, ID + 2, ID + 3],
            Some(&uri),
        )
        .await;
    });
    assert_labelled(&line, "airing", Some(4));
}

#[test]
fn the_id_bridges_say_which_feature_asked() {
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist::media_id_for_mal(&client(), ID, Some(&uri)).await;
    });
    assert_labelled(&line, "tracker_ids", Some(1));
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist::mal_id_for_media_id(&client(), ID, Some(&uri)).await;
    });
    assert_labelled(&line, "tracker_ids", Some(1));
    let line = one_line(|uri| async move {
        let _ =
            crate::meta::anilist::media_listed_for_mals(&client(), &[ID, ID + 1], Some(&uri)).await;
    });
    assert_labelled(&line, "watch_later_ids", Some(2));
    let line = one_line(|uri| async move {
        let _ = crate::meta::anilist::media_ids_recording_mal(&client(), ID, Some(&uri)).await;
    });
    assert_labelled(&line, "tracker_map", Some(1));
}

#[test]
fn the_account_requests_are_labelled_by_what_they_do() {
    let provider = |uri: &str| AniListProvider::with_bases(client(), uri.into(), uri.into());
    let line = one_line(|uri| async move {
        let _ = provider(&uri).me(&tokens()).await;
    });
    assert_labelled(&line, "account", None);
    let line = one_line(|uri| async move {
        let _ = provider(&uri).list_all_for(&tokens(), "1").await;
    });
    assert_labelled(&line, "user_list", None);
    let line = one_line(|uri| async move {
        let update = EntryUpdate {
            progress_episodes: Some(3),
            ..EntryUpdate::default()
        };
        let _ = provider(&uri)
            .update_entry(&tokens(), ProviderMediaId(ID), update)
            .await;
    });
    assert_labelled(&line, "list_write", Some(1));
    let line = one_line(|uri| async move {
        let _ = provider(&uri)
            .current_entry(&tokens(), ProviderMediaId(ID))
            .await;
    });
    assert_labelled(&line, "list_entry", Some(1));
    let line = one_line(|uri| async move {
        let _ = provider(&uri)
            .tracked_media(&tokens(), ProviderMediaId(ID))
            .await;
    });
    assert_labelled(&line, "tracked_media", Some(1));
    let line = one_line(|uri| async move {
        let _ = provider(&uri)
            .media_evidence(&tokens(), ProviderMediaId(ID))
            .await;
    });
    assert_labelled(&line, "media_evidence", Some(1));
}

#[test]
fn a_delete_labels_both_its_requests() {
    // The row-id lookup and the mutation it feeds are both the delete's.
    let lines = sent_lines_of(|| async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_string_contains("query MediaList("))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"data":{"MediaList":{"id":5}}}"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_string_contains("mutation Delete("))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":{"DeleteMediaListEntry":{"deleted":true}}}"#),
            )
            .mount(&server)
            .await;
        let provider = AniListProvider::with_bases(client(), server.uri(), server.uri());
        provider
            .delete_entry_for(&tokens(), ProviderMediaId(ID), "1")
            .await
            .expect("deleted");
    });
    assert_eq!(lines.len(), 2, "{lines:#?}");
    for line in &lines {
        assert_labelled(line, "list_delete", Some(1));
        assert!(!line.contains(&ID.to_string()), "an id in the log: {line}");
    }
}
