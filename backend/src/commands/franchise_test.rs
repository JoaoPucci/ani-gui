//! The franchise IO loop against a mocked Kitsu: each listing is read
//! once, paged where Kitsu counts more, and cached.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{franchise, listing_key};
use crate::app::AppState;
use crate::cache::meta_cache_put;

fn state_with_kitsu(kitsu_uri: &str) -> Arc<AppState> {
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

/// The detail row of the 2000 JoJo OVA, as the detail read caches it.
pub(crate) fn seed_ova_detail(state: &AppState) {
    let detail = json!({
        "id": "7730", "canonical_title": "JoJo no Kimyou na Bouken: Adventure",
        "titles": {}, "abbreviated_titles": [], "slug": null, "synopsis": null,
        "start_date": "2000-05-25", "end_date": "2002-11-01", "episode_count": 7,
        "average_rating": null, "subtype": "OVA", "status": "finished",
        "age_rating": null, "popularity_rank": null,
        "poster_image": { "tiny": null, "small": "s.jpg", "medium": "m.jpg", "large": null, "original": null },
        "cover_image": null
    });
    meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key("7730"),
        &detail.to_string(),
        3600,
    )
    .expect("seed detail");
}

/// An anime a relation points at: id, subtype, start.
type To = (&'static str, &'static str, &'static str);

fn page(rels: &[(&str, To)], count: usize) -> Value {
    let data: Vec<Value> = rels
        .iter()
        .map(|(role, (id, _, _))| {
            json!({ "type": "mediaRelationships", "attributes": { "role": role },
                "relationships": { "destination": { "data": { "type": "anime", "id": id } } } })
        })
        .collect();
    let included: Vec<Value> = rels
        .iter()
        .map(|(_, (id, subtype, start))| {
            json!({ "type": "anime", "id": id, "attributes": {
                "canonicalTitle": format!("T{id}"), "subtype": subtype, "status": "finished",
                "episodeCount": 12, "startDate": start, "posterImage": null, "coverImage": null } })
        })
        .collect();
    json!({ "data": data, "included": included, "meta": { "count": count } })
}

const TV_2012: To = ("7158", "TV", "2012-10-06");
const SC: To = ("8063", "TV", "2014-04-05");
const OVA_2000: To = ("7730", "OVA", "2000-05-25");
const OVA_1993: To = ("610", "OVA", "1993-11-19");
const FILM: To = ("3136", "movie", "2007-02-17");

async fn serve(server: &MockServer, id: &str, offset: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}/media-relationships")))
        .and(query_param("page[offset]", offset))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(server)
        .await;
}

/// From the 2000 OVA, the walk reads the OVA's, both neighbours' and
/// Stardust Crusaders' listings once each — the 2012 series' over two
/// pages — and a second ask is answered from the cache.
#[tokio::test]
async fn the_walk_reads_each_listing_once_and_pages() {
    let server = MockServer::start().await;
    serve(
        &server,
        "7730",
        "0",
        page(
            &[
                ("sequel", OVA_1993),
                ("alternative_version", SC),
                ("prequel", TV_2012),
            ],
            3,
        ),
    )
    .await;
    serve(&server, "610", "0", page(&[("prequel", OVA_2000)], 1)).await;
    serve(&server, "7158", "0", page(&[("sequel", SC)], 2)).await;
    serve(
        &server,
        "7158",
        "1",
        page(&[("alternative_version", FILM)], 2),
    )
    .await;
    serve(
        &server,
        "8063",
        "0",
        page(
            &[("prequel", TV_2012), ("alternative_version", OVA_2000)],
            2,
        ),
    )
    .await;
    let state = state_with_kitsu(&server.uri());
    seed_ova_detail(&state);

    let first = franchise(&state, "7730").await.expect("franchise");
    let ids: Vec<&str> = first.entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["610", "7730", "7158"]);
    let versions: Vec<&str> = first.versions.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(versions, ["3136", "8063"]);
    assert_eq!(
        first.entries[1].title,
        "JoJo no Kimyou na Bouken: Adventure"
    );
    assert_eq!(first.entries[1].poster.as_deref(), Some("m.jpg"));
    assert!(first.complete);

    let again = franchise(&state, "7730").await.expect("franchise");
    assert_eq!(first, again);
    assert!(
        crate::cache::meta_cache_get(&state.cache_pool, &listing_key("7158"))
            .expect("read")
            .is_some()
    );
}

/// A value that is not a Kitsu id asks Kitsu for nothing.
#[tokio::test]
async fn a_value_that_is_not_an_id_is_refused() {
    let state = state_with_kitsu("http://127.0.0.1:1");
    assert!(matches!(
        franchise(&state, "abc").await,
        Err(crate::error::AniError::InvalidKitsuId)
    ));
}
