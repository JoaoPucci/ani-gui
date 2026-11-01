//! A full-list refresh racing a tracker write: whatever order they land
//! in, the cached row is the tracker's latest answer for it. A child of
//! the `account_edit` tests, so it shares their wiremock helpers.

use super::{mal_provider, mappable_kitsu, mount_mal_patch, state_with_kitsu, test_tokens};
use crate::account::provider::{EntryUpdate, ProviderKind, ProviderMediaId};
use crate::account::status::ListStatus;

/// One planned show (MAL 21) as MAL lists it before any write.
const PLANNED_21: &str = r#"{"node":{"id":21,"title":"One Piece"},"list_status":{"status":"plan_to_watch","num_episodes_watched":0,"score":0,"updated_at":"2024-01-01T00:00:00+00:00"}}"#;
/// MAL 21 after the progress write below.
const WATCHING_21: &str = r#"{"node":{"id":21,"title":"One Piece"},"list_status":{"status":"watching","num_episodes_watched":3,"score":0,"updated_at":"2024-01-02T00:00:00+00:00"}}"#;

fn page(rows: &[&str]) -> String {
    format!(r#"{{"data":[{}],"paging":{{}}}}"#, rows.join(","))
}

/// A refresh whose list read went out before a watched episode's
/// progress write landed must not put the show back at Plan to Watch.
/// The first list answer is held until the write has cached its row;
/// any later list read answers with the tracker as it stands after it.
#[tokio::test]
async fn a_list_refresh_started_before_a_progress_write_cannot_undo_it() {
    use wiremock::matchers::{method, path};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    let list_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(page(&[WATCHING_21])))
        .with_priority(2)
        .mount(&list_server)
        .await;
    let (hold, gate) = crate::held::gate();
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me/animelist"))
        .respond_with(crate::held::Held::new(
            hold,
            wiremock::ResponseTemplate::new(200).set_body_string(page(&[PLANNED_21])),
        ))
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&list_server)
        .await;
    mount_mal_patch(&mal, None).await;
    let state = state_with_kitsu(&kitsu.uri());
    let kind = ProviderKind::MyAnimeList;
    let tokens = test_tokens();
    let provider = mal_provider(&mal.uri());
    let list_provider = mal_provider(&list_server.uri());
    let read = crate::commands::account::list_all_and_cache_via(
        &state,
        kind,
        &list_provider,
        &tokens,
        "4242",
    );
    let write = async {
        tokio::time::timeout(std::time::Duration::from_secs(5), gate.asked())
            .await
            .expect("the first list request reached its hold");
        crate::commands::account::write_through(
            &state,
            kind,
            &provider,
            &tokens,
            ProviderMediaId(21),
            EntryUpdate {
                status: Some(ListStatus::Watching),
                progress_episodes: Some(3),
                ..Default::default()
            },
            state.account_write_locks.epochs.current(kind),
        )
        .await
        .expect("the progress write lands");
        gate.release();
    };
    let (read, ()) = tokio::join!(read, write);
    read.expect("the refresh completes");
    let rows = crate::account::cache::list_entries(&state.cache_pool, kind, "4242").unwrap();
    let row = rows
        .iter()
        .find(|row| row.media_id.0 == 21)
        .expect("the written show stays cached");
    assert_eq!(
        (row.status, row.progress_episodes),
        (ListStatus::Watching, 3),
        "a snapshot read before the progress write undid it"
    );
}
