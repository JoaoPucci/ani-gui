//! The wire contract: what the backend sends the renderer and what it
//! reads back, published as `backend/wire-contract.json` for
//! `frontend/src/lib/wire-contract.test.ts` to hold the renderer's
//! hand-written types to. Nothing generates those types; this file and
//! that test are what keep the two sides saying the same thing.
//!
//! The file holds:
//!
//! - `responses`: per type the renderer reads, a `full` sample with every
//!   field the wire can carry, and a `min` sample with every field that
//!   can be left out left out (`None`, empty, `false`). A field in `min`
//!   is one the renderer always gets; a field in `full` alone is one it
//!   may not. A `full` sample that leaves a field out fails here
//!   ([`wire_probe::skipped_fields`]), so a field the renderer's type
//!   lacks cannot hide behind a sample that lacks it too.
//! - `requests`: per body the renderer sends, the fields the backend
//!   reads (`accepts`, from serde's own list) and those it cannot do
//!   without (`requires`: each is shown required by the body failing
//!   without it, and the rest optional by the body reading without
//!   them).
//! - `enums`: every value a string the renderer names can take.
//! - `errors`: the envelope of every error variant, and of the optional
//!   fields some variants carry.
//! - `constants` and `media_urls`: values the renderer keeps a copy of.
//!
//! Every list is built from the types themselves: a struct literal names
//! every field, and each enum list sits behind a `match` with no
//! wildcard, so a field or variant added here does not compile until
//! the samples say what it sends, and the published file then differs.
//!
//! What it does not hold: query strings (the renderer builds them as
//! text, with no type to compare), the search-page body (read by hand
//! in `commands::kitsu_search`, not through a type), the launch flags
//! (read by the Electron main process, not the renderer), and the value
//! types of request fields — a request's fields are held by name and by
//! whether they are required.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use super::account::account_wire::{
    AuthUrlRequest, AuthUrlResponse, EntryView, ExchangeCodeRequest, PkceWire, RefreshRequest,
    SetEntryRequest, TokensResponse, UpdateProgressRequest,
};
use super::refill::{Ended, WaitBody};
use super::wire_probe::{accepted_fields, skipped_fields};
use super::{ani_error_payload, HistoryRow, KitsuByMalIdsBody, SearchBody, TitleMatchBody};
use crate::account::pkce::PkceMethod;
use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId, UserProfile, UserStats};
use crate::account::status::ListStatus;
use crate::cache::reclaim::ReclaimReport;
use crate::commands::account::{status_to_snake, WATCH_LATER_BRIDGE_MAX_IDS};
use crate::commands::app_info::AppInfo;
use crate::commands::availability::{
    AvailabilityArgs, AvailabilityBatchArgs, AvailabilityBatchResponse, AvailabilityResponse,
    AvailabilityWarmArgs, UnsettledReason,
};
use crate::commands::continue_guess::RuledOutRequest;
use crate::commands::download::{DownloadArgs, DownloadProgress, DownloadResponse};
use crate::commands::external_player::{ExternalPlayerKind, LaunchArgs};
use crate::commands::history_number::HistoryNumber;
use crate::commands::play::PlayArgs;
use crate::commands::play_native_versions::{VersionInfo, VersionKind};
use crate::commands::progress::ProgressLine;
use crate::commands::session::{media_url, CreateSessionArgs, CreateSessionResponse};
use crate::config::Config;
use crate::error::AniError;
use crate::history::HistoryEntry;
use crate::meta::anilist_airing::{AiringStatus, UpcomingEpisode};
use crate::meta::aniskip::SkipInterval;
use crate::meta::github::ReleaseInfo;
use crate::meta::kitsu::{
    KitsuAnimeRef, KitsuCoverImage, KitsuEpisode, KitsuEpisodeThumbnail, KitsuPosterImage,
};
use crate::meta::kitsu_page::{KitsuSearchPage, SearchOrder};
use crate::proxy::{MediaKind, SessionSubtitle};
use crate::retired_provider::RetiredSweepReport;
use crate::scraper::provider::ProviderId;

/// One value per variant of an enum, each value written beside the
/// pattern that names its variant: the patterns make a `match` with no
/// wildcard, so a variant added does not compile until it has a pattern
/// here, and with it a value in the list. `except` names variants the
/// list leaves out on purpose.
macro_rules! listed {
    ($ty:ty { $($pat:pat => $val:expr),+ $(,)? } $(except $ex:pat)?) => {{
        #[allow(dead_code)]
        fn exhaustive(v: &$ty) {
            match v {
                $($pat => {})+
                $($ex => {})?
            }
        }
        vec![$($val),+]
    }};
}

/// [`listed!`] for an enum of unit variants.
macro_rules! units {
    ($ty:ident: $($v:ident),+ $(; except $ex:pat)?) => {
        listed!($ty { $($ty::$v => $ty::$v),+ } $(except $ex)?)
    };
}

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/wire-contract.json");

#[test]
fn the_published_wire_contract_is_what_the_backend_sends() {
    let want = format!(
        "{}\n",
        serde_json::to_string_pretty(&contract()).expect("the contract serializes")
    );
    let have = std::fs::read_to_string(PATH).unwrap_or_default();
    assert!(
        have == want,
        "{PATH} is not the backend's wire contract. Replace its contents with:\n{want}"
    );
}

fn contract() -> Value {
    json!({
        "responses": responses(),
        "requests": requests(),
        "enums": enums(),
        "errors": errors(),
        "constants": { "WATCH_LATER_BRIDGE_MAX_IDS": WATCH_LATER_BRIDGE_MAX_IDS },
        "media_urls": media_urls(),
    })
}

// — Responses ————————————————————————————————————————————————————————

/// A response type's two samples. `full` must leave no field out.
fn response<T: Serialize>(full: &T, min: &T) -> Value {
    let skipped = skipped_fields(full);
    assert!(
        skipped.is_empty(),
        "the full sample of {} leaves out {skipped:?}: give each a value",
        std::any::type_name::<T>()
    );
    json!({ "full": full, "min": min })
}

fn responses() -> Value {
    let mut out = Map::new();
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put("AppInfo", response(&app_info(true), &app_info(false)));
    put(
        "RetiredSweepReport",
        response(&retired_sweep(true), &retired_sweep(false)),
    );
    put("ReclaimReport", response(&reclaim(), &reclaim()));
    put(
        "HistoryRow",
        response(&history_row(true), &history_row(false)),
    );
    put(
        "HistoryEntry",
        response(&history_entry(true), &history_entry(false)),
    );
    put(
        "HistoryNumber",
        response(&history_number(true), &history_number(false)),
    );
    put(
        "CreateSessionResponse",
        response(&session(true), &session(false)),
    );
    put("SessionSubtitle", response(&subtitle(), &subtitle()));
    put("VersionInfo", response(&version(true), &version(false)));
    put("KitsuAnimeRef", response(&anime(true), &anime(false)));
    put("KitsuPosterImage", response(&poster(true), &poster(false)));
    put("KitsuCoverImage", response(&cover(true), &cover(false)));
    put("KitsuEpisode", response(&episode(true), &episode(false)));
    put(
        "KitsuEpisodeThumbnail",
        response(&thumbnail(true), &thumbnail(false)),
    );
    put(
        "KitsuSearchPage",
        response(&search_page(true), &search_page(false)),
    );
    put("SkipInterval", response(&skip(), &skip()));
    put("AiringStatus", response(&airing(true), &airing(false)));
    put("UpcomingEpisode", response(&upcoming(), &upcoming()));
    // No field of the config can be left out, so its one sample is both.
    // Not `Config::default()`: its Syncplay path differs per platform.
    put("Config", response(&config(), &config()));
    for line in every_progress_line() {
        let kind = serde_json::to_value(&line).expect("serializes")["kind"]
            .as_str()
            .expect("a progress line names its kind")
            .to_owned();
        put(&format!("ProgressLine.{kind}"), response(&line, &line));
    }
    put(
        "DownloadProgress",
        response(&download_progress(), &download_progress()),
    );
    put(
        "DownloadResponse",
        response(&download_response(), &download_response()),
    );
    put(
        "AvailabilityResponse",
        response(&availability(true), &availability(false)),
    );
    put(
        "AvailabilityBatchResponse",
        response(&availability_batch(true), &availability_batch(false)),
    );
    put("Ended", response(&ended(), &ended()));
    put("TokensResponse", response(&tokens(true), &tokens(false)));
    put("UserProfile", response(&profile(true), &profile(false)));
    put("UserStats", response(&stats(true), &stats(false)));
    put("ListEntry", response(&list_entry(true), &list_entry(false)));
    put("EntryView", response(&entry_view(), &entry_view()));
    put("AuthUrlResponse", response(&auth_url(), &auth_url()));
    put("ReleaseInfo", response(&release(), &release()));
    Value::Object(out)
}

fn strings(full: bool, s: &[&str]) -> Vec<String> {
    if full {
        s.iter().map(|&x| x.to_owned()).collect()
    } else {
        Vec::new()
    }
}

fn some<T>(full: bool, v: T) -> Option<T> {
    full.then_some(v)
}

fn app_info(full: bool) -> AppInfo {
    AppInfo {
        version: "0.18.0".into(),
        history_path: "/home/u/.local/state/ani-gui/ani-hsts".into(),
        proxy_base_url: "http://127.0.0.1:4100".into(),
        removed_legacy_paths: strings(full, &["/home/u/.cache/ani-gui/ani-cli"]),
        retired_provider_sweep: retired_sweep(full),
        cache_reclaim: reclaim(),
    }
}

fn retired_sweep(full: bool) -> RetiredSweepReport {
    RetiredSweepReport {
        superseded: strings(full, &["Frieren"]),
        renumbered: strings(full, &["One Piece"]),
        numbering_rows: 2,
        cache_rows: 3,
    }
}

fn reclaim() -> ReclaimReport {
    ReclaimReport {
        undeclared_rows: 4,
        expired_rows: 5,
    }
}

fn history_entry(full: bool) -> HistoryEntry {
    HistoryEntry {
        ep_no: "8".into(),
        id: "hianime:frieren-18542".into(),
        title: "Frieren".into(),
        watched_at: some(full, 1_700_000_000_000),
        kitsu_id: some(full, "46474".into()),
        own_number: true,
    }
}

fn history_row(full: bool) -> HistoryRow {
    HistoryRow {
        entry: history_entry(full),
        key_year: some(full, 2023),
    }
}

fn history_number(full: bool) -> HistoryNumber {
    HistoryNumber {
        ep_no: "12.5".into(),
        former_ep_no: some(full, "24.5".into()),
    }
}

fn session(full: bool) -> CreateSessionResponse {
    CreateSessionResponse {
        session_id: "3f1c".into(),
        media_url: "http://127.0.0.1:4100/s/3f1c/master.m3u8".into(),
        media_kind: MediaKind::Hls,
        cache_hit: full,
        subtitles: if full { vec![subtitle()] } else { Vec::new() },
        versions: if full {
            vec![version(true)]
        } else {
            Vec::new()
        },
        version: some(full, VersionKind::Uncensored),
        episode: some(full, "12.5".into()),
        former_episode: some(full, "24.5".into()),
        episode_unverified: full,
        own_listing: full,
    }
}

fn subtitle() -> SessionSubtitle {
    SessionSubtitle {
        lang: "en".into(),
        label: "English".into(),
        default: true,
        url: "http://127.0.0.1:4100/s/3f1c/sub/0.vtt".into(),
    }
}

fn version(full: bool) -> VersionInfo {
    VersionInfo {
        kind: VersionKind::Uncensored,
        show_key: "hianime:frieren-uncensored-18543".into(),
        has_mode: some(full, true),
    }
}

fn anime(full: bool) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: "46474".into(),
        canonical_title: "Frieren".into(),
        titles: if full {
            std::iter::once(("en_jp".to_owned(), "Sousou no Frieren".to_owned())).collect()
        } else {
            std::collections::HashMap::new()
        },
        abbreviated_titles: strings(full, &["Frieren: Beyond Journey's End"]),
        slug: some(full, "sousou-no-frieren".into()),
        synopsis: some(full, "An elf mage outlives her party.".into()),
        start_date: some(full, "2023-09-29".into()),
        end_date: some(full, "2024-03-22".into()),
        episode_count: some(full, 28),
        average_rating: some(full, 8.5),
        subtype: some(full, "TV".into()),
        status: some(full, "finished".into()),
        age_rating: some(full, "PG".into()),
        popularity_rank: some(full, 120),
        poster_image: some(full, poster(true)),
        cover_image: some(full, cover(true)),
        anilist_banner: some(full, "https://s4.anilist.co/file/banner.jpg".into()),
    }
}

fn poster(full: bool) -> KitsuPosterImage {
    KitsuPosterImage {
        tiny: some(full, "https://media.kitsu.app/p/tiny.jpg".into()),
        small: some(full, "https://media.kitsu.app/p/small.jpg".into()),
        medium: some(full, "https://media.kitsu.app/p/medium.jpg".into()),
        large: some(full, "https://media.kitsu.app/p/large.jpg".into()),
        original: some(full, "https://media.kitsu.app/p/original.jpg".into()),
    }
}

fn cover(full: bool) -> KitsuCoverImage {
    KitsuCoverImage {
        tiny: some(full, "https://media.kitsu.app/c/tiny.jpg".into()),
        small: some(full, "https://media.kitsu.app/c/small.jpg".into()),
        large: some(full, "https://media.kitsu.app/c/large.jpg".into()),
        original: some(full, "https://media.kitsu.app/c/original.jpg".into()),
    }
}

fn episode(full: bool) -> KitsuEpisode {
    KitsuEpisode {
        id: "270001".into(),
        canonical_title: some(full, "The Journey's End".into()),
        season_number: some(full, 1),
        number: some(full, 1),
        relative_number: some(full, 1),
        length: some(full, 24),
        synopsis: some(full, "The party returns.".into()),
        airdate: some(full, "2023-09-29".into()),
        thumbnail: some(full, thumbnail(true)),
    }
}

fn thumbnail(full: bool) -> KitsuEpisodeThumbnail {
    KitsuEpisodeThumbnail {
        original: some(full, "https://media.kitsu.app/e/original.jpg".into()),
    }
}

fn search_page(full: bool) -> KitsuSearchPage {
    KitsuSearchPage {
        hits: if full { vec![anime(true)] } else { Vec::new() },
        next_offset: some(full, 20),
    }
}

fn skip() -> SkipInterval {
    SkipInterval {
        skip_type: "op".into(),
        start_time: 90.5,
        end_time: 180.5,
    }
}

fn airing(full: bool) -> AiringStatus {
    AiringStatus {
        aired: some(full, 6),
        next_episode: some(full, 7),
        next_airing_at: some(full, 1_700_000_000),
        upcoming: if full { vec![upcoming()] } else { Vec::new() },
    }
}

fn upcoming() -> UpcomingEpisode {
    UpcomingEpisode {
        episode: 7,
        airing_at: 1_700_000_000,
    }
}

fn config() -> Config {
    Config {
        locale: "pt-BR".into(),
        mode: "dub".into(),
        download_quality: "720".into(),
        external_player: "vlc".into(),
        external_player_kind: ExternalPlayerKind::Vlc,
        external_player_custom_args: "--fullscreen".into(),
        syncplay_binary: "syncplay".into(),
        image_cache_cap_mb: 512,
        auto_play_next: true,
        download_bottom_bar_enabled: true,
        auto_skip_op: true,
        auto_skip_ed: true,
        use_custom_player_controls: true,
        update_include_prereleases: true,
        cache_resolutions: true,
        primary_account: "anilist".into(),
    }
}

fn every_progress_line() -> Vec<ProgressLine> {
    listed!(ProgressLine {
        ProgressLine::Searching { .. } => ProgressLine::Searching {
            provider: "hianime".into(),
        },
        ProgressLine::Matched { .. } => ProgressLine::Matched {
            title: "Frieren".into(),
        },
        ProgressLine::LinksFetched { .. } => ProgressLine::LinksFetched {
            provider: "hianime".into(),
        },
    })
}

fn download_progress() -> DownloadProgress {
    DownloadProgress {
        line: "[download] 42%".into(),
    }
}

fn download_response() -> DownloadResponse {
    DownloadResponse {
        dest_dir: "/home/u/Downloads/Frieren".into(),
    }
}

fn availability(full: bool) -> AvailabilityResponse {
    AvailabilityResponse {
        available: full,
        episode_count: some(full, 28),
        extra_episodes: strings(full, &["12.5"]),
        episode_count_approximate: full,
        gate_refused: full,
        provider: some(full, ProviderId::Hianime),
        unsettled: some(full, UnsettledReason::Refused),
        versions: if full {
            vec![version(true)]
        } else {
            Vec::new()
        },
    }
}

fn availability_batch(full: bool) -> AvailabilityBatchResponse {
    AvailabilityBatchResponse {
        cached: if full {
            std::iter::once(("46474".to_owned(), true)).collect()
        } else {
            std::collections::HashMap::new()
        },
        playable_episode_counts: if full {
            std::iter::once(("46474".to_owned(), 28)).collect()
        } else {
            std::collections::HashMap::new()
        },
    }
}

fn ended() -> Ended {
    Ended {
        ticket: 7,
        landed: true,
    }
}

fn tokens(full: bool) -> TokensResponse {
    TokensResponse {
        access_token: "at".into(),
        refresh_token: some(full, "rt".into()),
        expires_at_epoch_s: 1_700_000_000,
    }
}

fn stats(full: bool) -> UserStats {
    UserStats {
        anime_count: 120,
        mean_score_0_to_10: some(full, 7.5),
    }
}

fn profile(full: bool) -> UserProfile {
    UserProfile {
        provider: ProviderKind::AniList,
        user_id: "42".into(),
        username: "frieren".into(),
        avatar_url: some(full, "https://s4.anilist.co/file/avatar.png".into()),
        stats: some(full, stats(true)),
    }
}

fn list_entry(full: bool) -> ListEntry {
    ListEntry {
        provider: ProviderKind::MyAnimeList,
        media_id: ProviderMediaId(52991),
        mal_id: some(full, 52991),
        status: ListStatus::Watching,
        progress_episodes: 8,
        score_0_to_100: some(full, 90),
        updated_at_epoch_s: 1_700_000_000,
        title: "Frieren".into(),
    }
}

fn entry_view() -> EntryView {
    EntryView {
        status: status_to_snake(ListStatus::Watching).into(),
        progress: 8,
    }
}

fn auth_url() -> AuthUrlResponse {
    AuthUrlResponse {
        url: "https://myanimelist.net/v1/oauth2/authorize?state=s".into(),
    }
}

fn release() -> ReleaseInfo {
    ReleaseInfo {
        tag: "v0.18.0".into(),
        name: "0.18.0".into(),
        url: "https://github.com/JoaoPucci/ani-gui/releases/tag/v0.18.0".into(),
        published_at: "2026-11-13T00:00:00Z".into(),
        body: "Notes".into(),
    }
}

// — Requests —————————————————————————————————————————————————————————

/// A request body's fields. `min` must be exactly its required fields:
/// it reads as `T`, and stops reading with any one of them taken out.
fn request<T: DeserializeOwned>(min: &Value) -> Value {
    let name = std::any::type_name::<T>();
    let mut accepts = accepted_fields::<T>();
    accepts.sort_unstable();
    serde_json::from_value::<T>(min.clone())
        .unwrap_or_else(|e| panic!("{name} does not read its required fields alone: {e}"));
    let object = min.as_object().expect("a body is an object");
    let mut requires: Vec<&str> = object.keys().map(String::as_str).collect();
    for key in &requires {
        let mut without = object.clone();
        without.remove(*key);
        assert!(
            serde_json::from_value::<T>(Value::Object(without)).is_err(),
            "{name} reads without `{key}`: it is not required, leave it out of the sample"
        );
    }
    requires.sort_unstable();
    json!({ "accepts": accepts, "requires": requires })
}

fn requests() -> Value {
    let pkce = json!({ "verifier": "v", "challenge": "c", "method": "plain" });
    json!({
        "PlayArgs": request::<PlayArgs>(&json!({
            "title": "Frieren", "episode": "1", "mode": "sub"
        })),
        "DownloadArgs": request::<DownloadArgs>(&json!({
            "title": "Frieren", "episode": "1", "mode": "sub"
        })),
        "AvailabilityArgs": request::<AvailabilityArgs>(&json!({
            "title": "Frieren", "mode": "sub"
        })),
        "AvailabilityBatchArgs": request::<AvailabilityBatchArgs>(&json!({
            "kitsu_ids": ["46474"], "mode": "sub"
        })),
        "AvailabilityWarmArgs": request::<AvailabilityWarmArgs>(&json!({ "items": [] })),
        "LaunchArgs": request::<LaunchArgs>(&json!({
            "stream_url": "https://cdn.example/master.m3u8", "player_command": "mpv"
        })),
        "CreateSessionArgs": request::<CreateSessionArgs>(&json!({
            "upstream_url": "https://cdn.example/master.m3u8", "referer": "https://example/"
        })),
        "Config": request::<Config>(&json!({})),
        "RuledOutRequest": request::<RuledOutRequest>(&json!({
            "title": "Frieren", "episode": 8, "hits": []
        })),
        "TitleMatchBody": request::<TitleMatchBody>(&json!({
            "title": "Frieren", "cour": 1, "kitsu_id": "46474"
        })),
        "SearchBody": request::<SearchBody>(&json!({ "query": "frieren" })),
        "KitsuByMalIdsBody": request::<KitsuByMalIdsBody>(&json!({ "mal_ids": [52991] })),
        "WaitBody": request::<WaitBody>(&json!({ "tickets": [7] })),
        "AuthUrlRequest": request::<AuthUrlRequest>(&json!({ "state": "s", "pkce": pkce })),
        "ExchangeCodeRequest": request::<ExchangeCodeRequest>(&json!({
            "code": "c", "pkce": pkce
        })),
        "PkceWire": request::<PkceWire>(&pkce),
        "RefreshRequest": request::<RefreshRequest>(&json!({ "refresh_token": "rt" })),
        "UpdateProgressRequest": request::<UpdateProgressRequest>(&json!({ "kitsu_id": "46474" })),
        "SetEntryRequest": request::<SetEntryRequest>(&json!({ "kitsu_id": "46474" })),
    })
}

// — Enums ————————————————————————————————————————————————————————————

fn names<T: Serialize>(all: &[T]) -> Value {
    Value::Array(
        all.iter()
            .map(|v| serde_json::to_value(v).expect("serializes"))
            .collect(),
    )
}

fn enums() -> Value {
    json!({
        "MediaKind": names(&media_kinds()),
        "VersionKind": names(&version_kinds()),
        "UnsettledReason": names(&unsettled_reasons()),
        "ProviderId": names(&provider_ids()),
        "ExternalPlayerKind": names(&player_kinds()),
        "SearchOrder": names(&search_orders()),
        "ListStatus": names(&list_statuses()),
        "EntryView.status": list_statuses().into_iter().map(status_to_snake).collect::<Vec<_>>(),
        "ProviderKind": names(&provider_kinds()),
        "PkceMethod": pkce_methods().into_iter().map(PkceMethod::as_param).collect::<Vec<_>>(),
    })
}

fn media_kinds() -> Vec<MediaKind> {
    units!(MediaKind: Hls, Mp4)
}

fn version_kinds() -> Vec<VersionKind> {
    units!(VersionKind: Standard, Uncensored)
}

fn unsettled_reasons() -> Vec<UnsettledReason> {
    units!(UnsettledReason: Refused, Unanswered, PastAffinity, Superseded)
}

/// The providers a release walks: the test builds' stand-ins are not.
fn provider_ids() -> Vec<ProviderId> {
    units!(ProviderId: Hianime; except ProviderId::TestA | ProviderId::TestB)
}

fn player_kinds() -> Vec<ExternalPlayerKind> {
    units!(ExternalPlayerKind: Mpv, Vlc, Iina, Custom)
}

fn search_orders() -> Vec<SearchOrder> {
    units!(SearchOrder: Relevance, Year, Rating)
}

fn list_statuses() -> Vec<ListStatus> {
    units!(ListStatus: Planning, Watching, Completed, Paused, Dropped, Rewatching)
}

fn provider_kinds() -> Vec<ProviderKind> {
    units!(ProviderKind: AniList, MyAnimeList, InHouse)
}

fn pkce_methods() -> Vec<PkceMethod> {
    units!(PkceMethod: Plain, S256)
}

// — Errors, constants, URLs ——————————————————————————————————————————

/// The envelope of every variant, and of the variants whose optional
/// fields `every_variant` leaves at one setting, at the other.
fn errors() -> Value {
    let mut all = crate::error::variants::every_variant();
    all.push(AniError::StreamServerFailed {
        stage: "embed",
        host: None,
    });
    all.push(AniError::RateLimited {
        retry_after_secs: None,
        refused_by: Some("anilist"),
    });
    Value::Array(all.iter().map(ani_error_payload).collect())
}

fn media_urls() -> Value {
    let (base, session) = ("http://127.0.0.1:4100", "3f1c");
    json!({
        "base": base,
        "session": session,
        "urls": media_kinds()
            .into_iter()
            .map(|k| (serde_json::to_value(k).expect("serializes"), media_url(base, session, k)))
            .map(|(k, url)| (k.as_str().expect("a kind is a string").to_owned(), Value::String(url)))
            .collect::<Map<String, Value>>(),
    })
}
