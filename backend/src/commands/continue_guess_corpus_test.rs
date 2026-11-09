//! The regression corpus for a Continue row's guess, the backend's
//! half: which recorded hits each row rules out, and what the resolve
//! from its show key's words answers for the rows whose guess reaches
//! it. The renderer's half (`frontend/src/lib/history/
//! continue-guess-corpus.test.ts`) runs the guess over the same
//! recordings, takes the hits this half rules out as `ruled_out`
//! records them, and pins the pick. A rule that changes here changes
//! `ruled_out`, and with it the picks the renderer's half pins.

use super::*;
use crate::app::AppState;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(serde::Deserialize)]
struct Case {
    key: String,
    title: String,
    picks: BTreeMap<String, Option<String>>,
    /// The ids of the recorded hits the row rules out, by episode; an
    /// episode not listed rules none out.
    #[serde(default)]
    ruled_out: BTreeMap<String, Vec<String>>,
    /// What the resolve from the show key's words answers, for a row
    /// whose guess reaches it; absent for one whose guess does not.
    #[serde(default, deserialize_with = "present")]
    enrichment: Option<Option<String>>,
}

/// A field that is there, `null` or not: `Some(None)` for a `null`,
/// where an absent one stays `None`.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    <Option<String> as serde::Deserialize>::deserialize(d).map(Some)
}

#[derive(serde::Deserialize)]
struct Requests {
    search: BTreeMap<String, String>,
    slug: BTreeMap<String, String>,
    words: BTreeMap<String, String>,
}

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/continue-guess")
}

fn read<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&std::fs::read_to_string(dir().join(name)).expect(name)).expect(name)
}

fn hits(file: &str) -> Vec<KitsuAnimeRef> {
    crate::meta::kitsu::parse_search_response(&std::fs::read(dir().join(file)).expect(file))
        .expect("parses")
}

/// The slug the renderer looks a later season up by: lowercase,
/// every run of other characters one dash, none at the ends.
fn derived_slug(title: &str) -> String {
    title
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Every hit the renderer reads for `title`: its search's, and the
/// slug lookup's when it made one.
fn hits_for(req: &Requests, title: &str) -> Vec<KitsuAnimeRef> {
    let mut all = hits(req.search.get(title).expect("a recorded search"));
    if let Some(file) = req.slug.get(&derived_slug(title)) {
        all.extend(hits(file));
    }
    all
}

/// A recorded hit as the renderer would send it.
fn sent(h: &KitsuAnimeRef) -> SentHit {
    SentHit {
        id: h.id.clone(),
        canonical_title: h.canonical_title.clone(),
        titles: h.titles.clone(),
        abbreviated_titles: h.abbreviated_titles.clone(),
        slug: h.slug.clone(),
        episode_count: h.episode_count,
        subtype: h.subtype.clone(),
    }
}

#[test]
fn every_row_rules_out_the_hits_the_corpus_records() {
    let cases: Vec<Case> = read("cases.json");
    let req: Requests = read("requests.json");
    let mut wrong = Vec::new();
    for c in &cases {
        let all = hits_for(&req, &c.title);
        for ep in c.picks.keys() {
            let episode: f64 = ep.parse().expect("episode");
            let mut got = answer(RuledOutRequest {
                key: c.key.clone(),
                title: c.title.clone(),
                episode,
                hits: all.iter().map(sent).collect(),
            });
            got.dedup();
            let want = c.ruled_out.get(ep).cloned().unwrap_or_default();
            if got != want {
                wrong.push(format!(
                    "{} at {ep}: rules out {got:?}, corpus says {want:?}",
                    c.key
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

fn state_with_kitsu_at(uri: &str, history: PathBuf) -> AppState {
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: history,
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
    }
}

#[tokio::test]
async fn the_show_key_resolve_answers_what_the_corpus_records() {
    let cases: Vec<Case> = read("cases.json");
    let req: Requests = read("requests.json");
    let mut wrong = Vec::new();
    for c in cases.iter().filter(|c| c.enrichment.is_some()) {
        let term = crate::scraper::provider::ShowKey::parse(&c.key)
            .search_term()
            .expect("a slug-shaped key");
        let file = req
            .words
            .get(&term)
            .expect("a recorded search of the key's words");
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .and(query_param(
                "filter[text]",
                crate::meta::kitsu_page::search_text(&term),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(std::fs::read(dir().join(file)).expect(file.as_str())),
            )
            .mount(&mock)
            .await;
        let td = tempfile::tempdir().expect("tempdir");
        let state = state_with_kitsu_at(&mock.uri(), td.path().join("history"));
        let got = super::super::kitsu::resolve_allmanga_show_id(&state, &c.key, true)
            .await
            .expect("resolve")
            .map(|h| h.id);
        let want = c.enrichment.clone().flatten();
        if got != want {
            wrong.push(format!(
                "{}: resolves to {got:?}, corpus says {want:?}",
                c.key
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_corpus_holds_a_broad_sample() {
    let cases: Vec<Case> = read("cases.json");
    assert!(cases.len() > 100);
}
