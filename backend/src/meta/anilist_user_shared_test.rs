//! The account's identical queries sent at once share one request; its
//! writes never do.

use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::account::provider::{EntryUpdate, ProviderMediaId, Tokens, UserListProvider};
use crate::meta::anilist_user::AniListProvider;

fn bearer(token: &str) -> Tokens {
    Tokens {
        access_token: token.to_owned(),
        refresh_token: None,
        expires_at_epoch_s: 0,
    }
}

fn provider(server: &MockServer) -> AniListProvider {
    AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_owned(),
    )
}

async fn entry_read(server: &MockServer, times: u64) {
    Mock::given(method("POST"))
        .and(body_string_contains("mediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"data":{"Media":{"mediaListEntry":{"status":"CURRENT","progress":3}}}}"#,
                )
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .expect(times)
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_same_entry_read_for_one_bearer_at_once_is_one_request() {
    let server = MockServer::start().await;
    entry_read(&server, 1).await;
    let anilist = provider(&server);
    let tokens = bearer("tok-shared-0");
    let (a, b) = tokio::join!(
        anilist.current_entry(&tokens, ProviderMediaId(21)),
        anilist.current_entry(&tokens, ProviderMediaId(21)),
    );
    assert_eq!(a.expect("read"), b.expect("read"));
}

#[tokio::test]
async fn the_same_read_for_two_bearers_goes_out_for_each() {
    let server = MockServer::start().await;
    entry_read(&server, 2).await;
    let anilist = provider(&server);
    let (first, second) = (bearer("tok-shared-1"), bearer("tok-shared-1-b"));
    let (a, b) = tokio::join!(
        anilist.current_entry(&first, ProviderMediaId(21)),
        anilist.current_entry(&second, ProviderMediaId(21)),
    );
    a.expect("read");
    b.expect("read");
}

#[tokio::test]
async fn two_identical_writes_at_once_each_go_out() {
    // A write is never folded into another: each the user made is sent.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"data":{"SaveMediaListEntry":{"id":1,"mediaId":21,"status":"CURRENT","progress":3,"score":0,"updatedAt":1,"media":{"idMal":21,"title":{"romaji":"x","english":null}}}}}"#,
                )
                .set_delay(std::time::Duration::from_millis(100)),
        )
        .expect(2)
        .mount(&server)
        .await;
    let anilist = provider(&server);
    let tokens = bearer("tok-shared-2");
    let update = || EntryUpdate {
        progress_episodes: Some(3),
        ..Default::default()
    };
    let (a, b) = tokio::join!(
        anilist.update_entry(&tokens, ProviderMediaId(21), update()),
        anilist.update_entry(&tokens, ProviderMediaId(21), update()),
    );
    let _ = (a, b);
}

proptest::proptest! {
    /// A body is a write exactly when its query opens with `mutation`,
    /// whatever whitespace leads it; a query, or a body with no query
    /// text, is not.
    #[test]
    fn a_body_is_a_write_exactly_when_its_query_opens_with_mutation(
        lead in "[ \t\n]{0,3}",
        word in proptest::sample::select(vec!["mutation", "query", "Mutation", "subscription", ""]),
        rest in "[ A-Za-z{}()$:!]{0,20}",
    ) {
        let text = format!("{lead}{word}{rest}");
        let body = serde_json::json!({ "query": text });
        proptest::prop_assert_eq!(super::is_mutation(&body), word == "mutation");
    }

    #[test]
    fn a_body_without_query_text_is_not_a_write(n in proptest::num::i64::ANY) {
        let numeric = serde_json::json!({ "query": n });
        let no_query = serde_json::json!({ "variables": n });
        proptest::prop_assert!(!super::is_mutation(&numeric));
        proptest::prop_assert!(!super::is_mutation(&no_query));
    }
}

#[tokio::test]
async fn a_read_sent_after_a_write_never_takes_the_answer_from_before_it() {
    // The editor's re-read after a save, sent while an older read of the
    // same entry is still out, must reach the tracker: the older read's
    // answer may predate the write.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("mediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"data":{"Media":{"mediaListEntry":{"status":"CURRENT","progress":3}}}}"#,
                )
                .set_delay(std::time::Duration::from_millis(300)),
        )
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"SaveMediaListEntry":{"id":1,"mediaId":21,"status":"CURRENT","progress":4,"score":0,"updatedAt":1,"media":{"idMal":21,"title":{"romaji":"x","english":null}}}}}"#,
        ))
        .mount(&server)
        .await;
    let anilist = provider(&server);
    let tokens = bearer("tok-shared-3");
    let before = anilist.current_entry(&tokens, ProviderMediaId(21));
    let after = async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let _ = anilist
            .update_entry(
                &tokens,
                ProviderMediaId(21),
                EntryUpdate {
                    progress_episodes: Some(4),
                    ..Default::default()
                },
            )
            .await;
        anilist.current_entry(&tokens, ProviderMediaId(21)).await
    };
    let (a, b) = tokio::join!(before, after);
    a.expect("read");
    b.expect("read");
    // `.expect(2)`: the read after the write went out on its own.
}

/// AniList answering the account's reads, with the `Viewer` query
/// expected `viewers` times.
async fn anilist_account(server: &MockServer, viewers: u64) {
    Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Viewer":{"id":4242,"name":"shiro","avatar":{"large":null,"medium":null},"statistics":{"anime":{"count":0,"meanScore":0}}}}}"#,
        ))
        .expect(viewers)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("MediaListCollection"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"MediaListCollection":{"hasNextChunk":false,"lists":[]}}}"#,
        ))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("$mediaId: Int!, $userId: Int!"))
        .and(body_string_contains("\"userId\":4242"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"MediaList":{"id":7777}}}"#),
        )
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("DeleteMediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"DeleteMediaListEntry":{"deleted":true}}}"#),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_list_pull_for_a_known_user_asks_anilist_nothing_about_the_viewer() {
    let server = MockServer::start().await;
    anilist_account(&server, 0).await;
    provider(&server)
        .list_all_for(&bearer("tok-shared-4"), "4242")
        .await
        .expect("pulled");
}

#[tokio::test]
async fn a_removal_for_a_known_user_asks_anilist_nothing_about_the_viewer() {
    let server = MockServer::start().await;
    anilist_account(&server, 0).await;
    provider(&server)
        .delete_entry_for(&bearer("tok-shared-5"), ProviderMediaId(21), "4242")
        .await
        .expect("removed");
}

#[tokio::test]
async fn a_list_pull_and_a_removal_with_no_known_user_still_ask_for_it() {
    let server = MockServer::start().await;
    anilist_account(&server, 2).await;
    let anilist = provider(&server);
    anilist
        .list_all(&bearer("tok-shared-6"))
        .await
        .expect("pulled");
    anilist
        .delete_entry(&bearer("tok-shared-6"), ProviderMediaId(21))
        .await
        .expect("removed");
}

proptest::proptest! {
    /// AniList's user id reads back as the number it is, and anything
    /// that is not one is a parse failure, never a user.
    #[test]
    fn a_viewer_id_reads_as_its_number_and_nothing_else_does(n in proptest::num::i64::ANY, junk in "[a-z ]{1,8}") {
        proptest::prop_assert_eq!(super::viewer_id(&n.to_string()).ok(), Some(n));
        proptest::prop_assert!(super::viewer_id(&junk).is_err());
    }
}

/// AniList answering the entry read after `read_ms`, and the save after
/// `write_ms`; the read expected `reads` times.
async fn slow_account(server: &MockServer, read_ms: u64, write_ms: u64, reads: u64) {
    Mock::given(method("POST"))
        .and(body_string_contains("mediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{"data":{"Media":{"mediaListEntry":{"status":"CURRENT","progress":3}}}}"#,
                )
                .set_delay(std::time::Duration::from_millis(read_ms)),
        )
        .expect(reads)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"SaveMediaListEntry":{"id":1,"mediaId":21,"status":"CURRENT","progress":4,"score":0,"updatedAt":1,"media":{"idMal":21,"title":{"romaji":"x","english":null}}}}}"#)
                .set_delay(std::time::Duration::from_millis(write_ms)),
        )
        .mount(server)
        .await;
}

fn save() -> EntryUpdate {
    EntryUpdate {
        progress_episodes: Some(4),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_read_sent_after_a_write_answered_never_joins_one_sent_while_it_was_out() {
    // R1 goes out while the save is at AniList and may see the row from
    // before it; R2 goes out after the save was answered and must not
    // take R1's answer.
    let server = MockServer::start().await;
    slow_account(&server, 600, 300, 2).await;
    let anilist = provider(&server);
    let tokens = bearer("tok-during-flight");
    let write = anilist.update_entry(&tokens, ProviderMediaId(21), save());
    let during = async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        anilist.current_entry(&tokens, ProviderMediaId(21)).await
    };
    let after = async {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        anilist.current_entry(&tokens, ProviderMediaId(21)).await
    };
    let (w, a, b) = tokio::join!(write, during, after);
    w.expect("saved");
    a.expect("read");
    b.expect("read");
}

#[tokio::test]
async fn a_write_dropped_while_out_still_parts_the_reads_before_and_after_it() {
    // The request went out and may land; the caller stopped waiting. A
    // read sent after that must not take the answer of one sent while
    // the write was out.
    let server = MockServer::start().await;
    slow_account(&server, 600, 400, 2).await;
    let anilist = provider(&server);
    let tokens = bearer("tok-dropped-write");
    let write = async {
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            anilist.update_entry(&tokens, ProviderMediaId(21), save()),
        )
        .await;
    };
    let during = async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        anilist.current_entry(&tokens, ProviderMediaId(21)).await
    };
    let after = async {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        anilist.current_entry(&tokens, ProviderMediaId(21)).await
    };
    let ((), a, b) = tokio::join!(write, during, after);
    a.expect("read");
    b.expect("read");
}
