//! A full-list refresh racing a tracker write: whatever order they land
//! in, the cached row is the tracker's latest answer for it. A child of
//! the `account_edit` tests, so it shares their wiremock helpers.

use super::{
    mal_patch_answer, mal_provider, mappable_kitsu, mount_mal_patch, remove_entry_via,
    set_entry_via, state_with_kitsu, test_tokens,
};
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

/// Which write a case makes.
#[derive(Clone, Copy, Debug)]
enum Write {
    Save,
    Removal,
    Progress,
}

/// MAL 21 cached at Plan to Watch for user 4242, the tracker's owner
/// lookup failing with a 500 — every time, or only the first time — and
/// `write` made. The write must still go out; the cached row must then
/// be the write's answer, or, with no owner known to cache it under,
/// gone rather than left at Plan to Watch; and a refresh made after it
/// must fill the row in again from the tracker.
async fn owner_lookup_fails(write: Write, fails_again: bool) {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    let state = state_with_kitsu(&kitsu.uri());
    let kind = ProviderKind::MyAnimeList;
    let planned = crate::account::provider::ListEntry {
        provider: kind,
        media_id: ProviderMediaId(21),
        mal_id: Some(21),
        status: ListStatus::Planning,
        progress_episodes: 0,
        score_0_to_100: None,
        updated_at_epoch_s: 1,
        title: "One Piece".into(),
    };
    crate::account::cache::write_entries(&state.cache_pool, kind, "4242", &[planned]).unwrap();
    let removal = matches!(write, Write::Removal);
    let mutated = Arc::new(AtomicBool::new(false));
    let wrote = mutated.clone();
    wiremock::Mock::given(method(if removal { "DELETE" } else { "PATCH" }))
        .and(path("/anime/21/my_list_status"))
        .respond_with(move |_: &wiremock::Request| {
            wrote.store(true, Ordering::SeqCst);
            if removal {
                wiremock::ResponseTemplate::new(200)
            } else {
                mal_patch_answer()
            }
        })
        .mount(&mal)
        .await;
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(move |_: &wiremock::Request| {
            if count.fetch_add(1, Ordering::SeqCst) == 0 || fails_again {
                wiremock::ResponseTemplate::new(500)
            } else {
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"id":4242,"name":"shiro"}"#)
            }
        })
        .mount(&mal)
        .await;
    let provider = mal_provider(&mal.uri());
    let tokens = test_tokens();
    let update = EntryUpdate {
        status: Some(ListStatus::Watching),
        progress_episodes: Some(3),
        ..Default::default()
    };
    let result = match write {
        Write::Progress => crate::commands::account::write_through(
            &state,
            kind,
            &provider,
            &tokens,
            ProviderMediaId(21),
            update,
            state.account_write_locks.epochs.current(kind),
        )
        .await
        .map(|_| ()),
        Write::Removal => remove_entry_via(&state, kind, &provider, &tokens, "12")
            .await
            .map(|_| ()),
        Write::Save => set_entry_via(&state, kind, &provider, &tokens, "12", update)
            .await
            .map(|_| ()),
    };
    result.unwrap_or_else(|e| panic!("{write:?}: the write failed: {e:?}"));
    assert!(
        mutated.load(Ordering::SeqCst),
        "{write:?}: a failed owner lookup kept the write from the tracker"
    );
    let rows = crate::account::cache::list_entries(&state.cache_pool, kind, "4242").unwrap();
    let row = rows.iter().find(|r| r.media_id.0 == 21);
    match (removal, fails_again) {
        (false, false) => {
            let row = row.unwrap_or_else(|| panic!("{write:?}: the written row is not cached"));
            assert_eq!(
                (row.status, row.progress_episodes),
                (ListStatus::Watching, 3),
                "{write:?}: the cache missed the write"
            );
        }
        _ => assert!(
            row.is_none(),
            "{write:?}: the cached row was left stale: {row:?}"
        ),
    }
    // A refresh made afterwards reads the row from the tracker again.
    let list = wiremock::MockServer::start().await;
    let after = if removal { "" } else { WATCHING_21 };
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me/animelist"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(page(&[after])))
        .mount(&list)
        .await;
    crate::commands::account::list_all_and_cache_via(
        &state,
        kind,
        &mal_provider(&list.uri()),
        &tokens,
        "4242",
    )
    .await
    .expect("the refresh lands");
    let rows = crate::account::cache::list_entries(&state.cache_pool, kind, "4242").unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r.media_id.0 == 21)
            .map(|r| (r.status, r.progress_episodes)),
        (!removal).then_some((ListStatus::Watching, 3)),
        "{write:?}: the next refresh did not fill the row in"
    );
}

#[tokio::test]
async fn a_save_goes_out_when_the_owner_lookup_fails_and_is_cached_once_it_answers() {
    owner_lookup_fails(Write::Save, false).await;
}
#[tokio::test]
async fn a_save_goes_out_when_the_owner_lookup_keeps_failing_and_drops_the_stale_row() {
    owner_lookup_fails(Write::Save, true).await;
}
#[tokio::test]
async fn a_removal_goes_out_when_the_owner_lookup_fails_and_is_cached_once_it_answers() {
    owner_lookup_fails(Write::Removal, false).await;
}
#[tokio::test]
async fn a_removal_goes_out_when_the_owner_lookup_keeps_failing_and_drops_the_stale_row() {
    owner_lookup_fails(Write::Removal, true).await;
}
#[tokio::test]
async fn progress_goes_out_when_the_owner_lookup_fails_and_is_cached_once_it_answers() {
    owner_lookup_fails(Write::Progress, false).await;
}
#[tokio::test]
async fn progress_goes_out_when_the_owner_lookup_keeps_failing_and_drops_the_stale_row() {
    owner_lookup_fails(Write::Progress, true).await;
}
