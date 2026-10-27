//! The episode map for the shows a special filed among a tracker
//! entry's extras touches, against the bodies Kitsu and AniList served
//! for them: Fate/stay night: Unlimited Blade Works, whose prologue
//! special (Kitsu 9718) AniList files as episode 0 of the TV season's
//! entry (19603); and Seikaisuru Kado, whose two specials AniList lists
//! beside the season it counts one episode longer than Kitsu does.

use super::*;

const UBW_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-19603.json");
const UBW_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-7882.json");
const UBW_PROLOGUE_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-9718.json");
const UBW_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-7882.json");
const UBW_PROLOGUE_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-9718.json");
const UBW_MAPPED_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-19603.json");
const UBW_MAPPED_MAL: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-22297.json");
const KADO_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-21452.json");
const KADO_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-11490.json");
const KADO_NINOVO_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-13323.json");
const KADO_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-11490.json");
const KADO_NINOVO_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-13323.json");
const KADO_MAPPED_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-21452.json");
const KADO_MAPPED_MAL: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-32032.json");

const KAMITSUBAKI_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-173335.json");
const KAMITSUBAKI_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-49900.json");
const KAMITSUBAKI_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-49900.json");
const ZENPEN_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-50027.json");
const KAMITSUBAKI_MAPPED_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-173335.json");
const KAMITSUBAKI_MAPPED_MAL: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-57820.json");

const FATE_ZERO_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/anilist-media-11741.json");
const FATE_ZERO_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-7658.json");
const FATE_ZERO_RELATIONS: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-relationships-7658.json");
const FATE_ZERO_MAPPED_ANILIST: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-anilist-11741.json");
const FATE_ZERO_MAPPED_MAL: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-mappings-mal-11741.json");
const ZENPEN_KITSU: &str =
    include_str!("../../../tests/fixtures/tracker-map/wire/kitsu-anime-50027.json");

/// AniList's evidence for `id`, as recorded, however often it is read.
async fn evidence_of(server: &MockServer, id: u32, body: &str) {
    Mock::given(method("POST"))
        .and(body_string_contains("streamingEpisodes"))
        .and(body_string_contains(format!("\"mediaId\":{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Kitsu's detail for `id`, as recorded, however often it is read.
async fn detail_of(kitsu: &MockServer, id: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}")))
        .and(query_param("fields[anime]", ANIME_FIELDS))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(kitsu)
        .await;
}

/// Kitsu's `/anime/:id?include=mappings` for `id`, mapped as given.
async fn mapped_as(kitsu: &MockServer, id: &str, mal: Option<u32>, anilist: Option<u32>) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}")))
        .and(query_param("include", "mappings"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mappings(id, mal, anilist)))
        .mount(kitsu)
        .await;
}

async fn relations_of(kitsu: &MockServer, id: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}/media-relationships")))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(kitsu)
        .await;
}

/// Kitsu as it answers for Unlimited Blade Works and the entries the
/// map walks to from it: the prologue special (MAL 27821 only, which
/// AniList does not know), Fate/Zero's second season before it, and the
/// TV series' own second season after.
async fn ubw_kitsu() -> MockServer {
    let kitsu = MockServer::start().await;
    detail_of(&kitsu, "7882", UBW_KITSU).await;
    detail_of(&kitsu, "9718", UBW_PROLOGUE_KITSU).await;
    relations_of(&kitsu, "7882", UBW_RELATIONS).await;
    relations_of(&kitsu, "9718", UBW_PROLOGUE_RELATIONS).await;
    mapped_as(&kitsu, "7882", Some(22_297), Some(19_603)).await;
    mapped_as(&kitsu, "9718", Some(27_821), None).await;
    mapped_as(&kitsu, "7658", Some(11_741), Some(11_741)).await;
    mapped_as(&kitsu, "10052", Some(28_701), Some(20_792)).await;
    kitsu_mapped(&kitsu, "anilist/anime", 19_603, UBW_MAPPED_ANILIST).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 22_297, UBW_MAPPED_MAL).await;
    kitsu
}

/// AniList as it answers for 19603, and the id translations: MAL
/// 22297 is 19603, MAL 27821 is nothing it knows.
async fn ubw_anilist() -> MockServer {
    let server = MockServer::start().await;
    anilist_media(&server, 19_603, UBW_ANILIST).await;
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("\"idMal\":22297"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":19603}}}"#),
        )
        .mount(&server)
        .await;
    server
}

fn ask<'a>(
    state: &'a Arc<AppState>,
    kind: ProviderKind,
    provider: &'a dyn UserListProvider,
    tokens: &'a Tokens,
    anilist_base: &'a str,
) -> MapAsk<'a> {
    MapAsk {
        state,
        kind,
        provider,
        tokens,
        anilist_base: Some(anilist_base),
    }
}

/// The TV season on AniList, read from what both services served: the
/// prologue AniList lists as "Episode 0" shifts every episode one on,
/// and the finale sends 19603's total, completed.
#[tokio::test]
async fn unlimited_blade_works_tv_reads_its_prologue_as_an_extra_on_anilist() {
    let kitsu = ubw_kitsu().await;
    let anilist_server = ubw_anilist().await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let map = episode_map_for(
        &ask(
            &state,
            ProviderKind::AniList,
            &provider,
            &tokens,
            &anilist_server.uri(),
        ),
        "7882",
        ProviderMediaId(19_603),
        Some(13),
    )
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::Extras, "{map:?}");
    assert_eq!(
        map.segments
            .iter()
            .map(|s| (
                s.entry,
                s.kitsu_from,
                s.kitsu_to,
                s.lead,
                s.extras_after.clone()
            ))
            .collect::<Vec<_>>(),
        [(19_603, 1, 12, 0, vec![0])]
    );
    let write = |k| match map.target(k) {
        crate::account::episode_map::Target::Mapped(w) => Some((w.entry, w.progress, w.complete)),
        crate::account::episode_map::Target::Identity => None,
    };
    assert_eq!(write(1), Some((19_603, 2, false)));
    assert_eq!(write(11), Some((19_603, 12, false)));
    assert_eq!(write(12), Some((19_603, 13, true)));
}

/// On MyAnimeList the prologue has an entry of its own (27821), one
/// episode against Kitsu's one: the counts agree and the map asks the
/// tracker nothing.
#[tokio::test]
async fn unlimited_blade_works_prologue_on_mal_is_its_own_entry() {
    let kitsu = ubw_kitsu().await;
    let unused = MockServer::start().await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&unused.uri());
    let tokens = tokens();
    let map = episode_map_for(
        &ask(
            &state,
            ProviderKind::MyAnimeList,
            &provider,
            &tokens,
            &unused.uri(),
        ),
        "9718",
        ProviderMediaId(27_821),
        Some(1),
    )
    .await
    .expect("the map");
    assert_eq!(map, EpisodeMap::identity(Identity::CountsAgree));
    assert!(unused
        .received_requests()
        .await
        .unwrap_or_default()
        .is_empty());
}

/// Seikaisuru Kado's season: AniList counts 13 against Kitsu's 12 and
/// lists both an "Episode 0" and an "Episode 6.5", two extras for a
/// surplus of one. Nothing says which it counts, so the season's map is
/// identity — and so is that of its prologue special, Ninovo, which
/// AniList has no entry for.
#[tokio::test]
async fn seikaisuru_kado_and_its_prologue_special_stay_identity_on_anilist() {
    let kitsu = MockServer::start().await;
    detail_of(&kitsu, "11490", KADO_KITSU).await;
    detail_of(&kitsu, "13323", KADO_NINOVO_KITSU).await;
    relations_of(&kitsu, "11490", KADO_RELATIONS).await;
    relations_of(&kitsu, "13323", KADO_NINOVO_RELATIONS).await;
    mapped_as(&kitsu, "11490", Some(32_032), Some(21_452)).await;
    mapped_as(&kitsu, "13323", Some(35_369), None).await;
    mapped_as(&kitsu, "13523", Some(35_603), None).await;
    mapped_as(&kitsu, "41390", None, None).await;
    kitsu_mapped(&kitsu, "anilist/anime", 21_452, KADO_MAPPED_ANILIST).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 32_032, KADO_MAPPED_MAL).await;
    let anilist_server = MockServer::start().await;
    evidence_of(&anilist_server, 21_452, KADO_ANILIST).await;
    no_anilist_answer(&anilist_server).await;
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("\"idMal\":32032"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":21452}}}"#),
        )
        .mount(&anilist_server)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let base = anilist_server.uri();
    let ask = ask(&state, ProviderKind::AniList, &provider, &tokens, &base);
    let season = episode_map_for(&ask, "11490", ProviderMediaId(21_452), Some(13))
        .await
        .expect("the season's map");
    assert_eq!(season, EpisodeMap::identity(Identity::Unexplained));
    let special = episode_map_for_unresolved(&ask, "13323")
        .await
        .expect("the special's map");
    assert!(
        special.segments.is_empty(),
        "the special writes nothing: {special:?}"
    );
}

/// Kitsu and AniList as they answer for Kamitsubaki-shi Kensetsuchuu.:
/// the series (49900, MAL 57820, AniList 173335) and the one-episode
/// special Kitsu chains after it, Witchling - Zenpen (50027, MAL 61971
/// only, which AniList does not know). AniList also answers for the
/// banner of a show Kitsu has no cover for, with nothing.
async fn kamitsubaki() -> (MockServer, MockServer) {
    let kitsu = MockServer::start().await;
    detail_of(&kitsu, "49900", KAMITSUBAKI_KITSU).await;
    relations_of(&kitsu, "49900", KAMITSUBAKI_RELATIONS).await;
    relations_of(&kitsu, "50027", ZENPEN_RELATIONS).await;
    mapped_as(&kitsu, "49900", Some(57_820), Some(173_335)).await;
    mapped_as(&kitsu, "50027", Some(61_971), None).await;
    kitsu_mapped(&kitsu, "anilist/anime", 173_335, KAMITSUBAKI_MAPPED_ANILIST).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 57_820, KAMITSUBAKI_MAPPED_MAL).await;
    let anilist_server = MockServer::start().await;
    evidence_of(&anilist_server, 173_335, KAMITSUBAKI_ANILIST).await;
    no_anilist_answer(&anilist_server).await;
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .and(body_string_contains("\"idMal\":57820"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":173335}}}"#),
        )
        .mount(&anilist_server)
        .await;
    (kitsu, anilist_server)
}

/// The special Kitsu chains after Kamitsubaki's series aired six days
/// before it, as the relations Kitsu serves date it: no later part. The
/// series' map is AniList's listed Episode 0 alone — Kitsu's 1 sends 2,
/// its finale sends 173335's 13, completed — where reading the special
/// as the 13th episode made it a conflict.
#[tokio::test]
async fn kamitsubaki_s_series_reads_its_episode_0_on_anilist() {
    let (kitsu, anilist_server) = kamitsubaki().await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let base = anilist_server.uri();
    let map = episode_map_for(
        &ask(&state, ProviderKind::AniList, &provider, &tokens, &base),
        "49900",
        ProviderMediaId(173_335),
        Some(13),
    )
    .await
    .expect("the map");
    assert_eq!(map.shape, Shape::Extras, "{map:?}");
    let write = |k| match map.target(k) {
        crate::account::episode_map::Target::Mapped(w) => Some((w.entry, w.progress, w.complete)),
        crate::account::episode_map::Target::Identity => None,
    };
    assert_eq!(write(1), Some((173_335, 2, false)));
    assert_eq!(write(12), Some((173_335, 13, true)));
}

/// Unlimited Blade Works' prologue (Kitsu 9718) has no AniList entry:
/// Kitsu maps it to MAL 27821 alone, which AniList does not know. AniList
/// counts it as the "Episode 0" of the TV season's 19603, and the
/// season's own map reads it so: the prologue is 19603's first episode,
/// never its last. Through its prequel, Fate/Zero's second season, it
/// is filed nowhere, as before.
#[tokio::test]
async fn unlimited_blade_works_prologue_is_the_tv_season_s_first_episode_on_anilist() {
    let kitsu = ubw_kitsu().await;
    detail_of(&kitsu, "7658", FATE_ZERO_KITSU).await;
    relations_of(&kitsu, "7658", FATE_ZERO_RELATIONS).await;
    mapped_as(&kitsu, "6028", Some(10_087), Some(10_087)).await;
    mapped_as(&kitsu, "4788", Some(6922), Some(6922)).await;
    mapped_as(&kitsu, "8675", Some(25_537), Some(20_791)).await;
    kitsu_mapped(&kitsu, "anilist/anime", 11_741, FATE_ZERO_MAPPED_ANILIST).await;
    kitsu_mapped(&kitsu, "myanimelist/anime", 11_741, FATE_ZERO_MAPPED_MAL).await;
    let anilist_server = MockServer::start().await;
    evidence_of(&anilist_server, 19_603, UBW_ANILIST).await;
    evidence_of(&anilist_server, 11_741, FATE_ZERO_ANILIST).await;
    no_anilist_answer(&anilist_server).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let base = anilist_server.uri();
    let ask = ask(&state, ProviderKind::AniList, &provider, &tokens, &base);
    let map = episode_map_for_unresolved(&ask, "9718")
        .await
        .expect("the map");
    assert_eq!(map.shape, Shape::SpecialExtras, "{map:?}");
    match map.target(1) {
        crate::account::episode_map::Target::Mapped(w) => {
            assert_eq!((w.entry, w.progress, w.complete), (19_603, 1, false));
            assert!(w.earlier.is_empty());
        }
        crate::account::episode_map::Target::Identity => panic!("filed nowhere"),
    }
    // The TV season's own map is the one it read, and unchanged.
    let season = episode_map_for(&ask, "7882", ProviderMediaId(19_603), Some(13))
        .await
        .expect("the season's map");
    assert_eq!(season.shape, Shape::Extras);
    assert_eq!(season.segments[0].extras_after, [0]);
}

/// Kamitsubaki's special, Witchling - Zenpen (Kitsu 50027), has no
/// AniList entry. Kitsu chains it after the series, but it aired six
/// days before the series began, and the series' entry lists an Episode
/// 0: the special is that entry's first episode.
#[tokio::test]
async fn kamitsubaki_s_special_is_its_series_entry_s_first_episode_on_anilist() {
    let (kitsu, anilist_server) = kamitsubaki().await;
    detail_of(&kitsu, "50027", ZENPEN_KITSU).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let base = anilist_server.uri();
    let ask = ask(&state, ProviderKind::AniList, &provider, &tokens, &base);
    let map = episode_map_for_unresolved(&ask, "50027")
        .await
        .expect("the map");
    assert_eq!(map.shape, Shape::SpecialExtras, "{map:?}");
    match map.target(1) {
        crate::account::episode_map::Target::Mapped(w) => {
            assert_eq!((w.entry, w.progress, w.complete), (173_335, 1, false));
        }
        crate::account::episode_map::Target::Identity => panic!("filed nowhere"),
    }
}

/// A one-episode special Kitsu maps to nothing, related only to UBW's TV
/// season, as Kitsu serves it with `relations` for the special's page.
async fn lone_special(kitsu: &MockServer, relations: &str) {
    detail_of(
        kitsu,
        "90001",
        &UBW_PROLOGUE_KITSU.replace("\"9718\"", "\"90001\""),
    )
    .await;
    relations_of(kitsu, "90001", relations).await;
    mapped_as(kitsu, "90001", None, None).await;
}

/// The special's page relating it to UBW's TV season alone.
fn sequel_7882(count: usize) -> String {
    format!(
        r#"{{"data":[{{"id":"r1","type":"mediaRelationships","attributes":{{"role":"sequel"}},
            "relationships":{{"destination":{{"data":{{"type":"anime","id":"7882"}}}}}}}}],
          "included":[{{"id":"7882","type":"anime","attributes":{{"subtype":"TV","status":"finished",
            "episodeCount":12,"startDate":"2014-10-12"}}}}],
          "meta":{{"count":{count}}}}}"#
    )
}

/// On MyAnimeList the special rule is never asked: a show Kitsu maps to
/// nothing there, with no prequel, writes nothing, and the tracker is
/// asked nothing.
#[tokio::test]
async fn a_show_unresolved_on_mal_is_never_asked_about_as_a_special() {
    let kitsu = ubw_kitsu().await;
    lone_special(&kitsu, &sequel_7882(1)).await;
    let mal_server = MockServer::start().await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal(&mal_server.uri());
    let tokens = tokens();
    let base = mal_server.uri();
    let map = episode_map_for_unresolved(
        &ask(&state, ProviderKind::MyAnimeList, &provider, &tokens, &base),
        "90001",
    )
    .await
    .expect("the map");
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    assert!(mal_server
        .received_requests()
        .await
        .unwrap_or_default()
        .is_empty());
}

/// A page of the special's relations that did not hold them all says
/// nothing of which entry is the only one: it is filed nowhere.
#[tokio::test]
async fn a_special_whose_relations_kitsu_did_not_list_whole_is_filed_nowhere() {
    let kitsu = ubw_kitsu().await;
    lone_special(&kitsu, &sequel_7882(2)).await;
    let anilist_server = MockServer::start().await;
    no_anilist_answer(&anilist_server).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = anilist(&anilist_server.uri());
    let tokens = tokens();
    let base = anilist_server.uri();
    let map = episode_map_for_unresolved(
        &ask(&state, ProviderKind::AniList, &provider, &tokens, &base),
        "90001",
    )
    .await
    .expect("the map");
    assert_eq!(map, EpisodeMap::identity(Identity::Unexplained));
    // No related entry was read for its extras.
    let evidence_reads = anilist_server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| String::from_utf8_lossy(&r.body).contains("streamingEpisodes"))
        .count();
    assert_eq!(evidence_reads, 0);
}

/// A related entry the tracker answers is gone holds nothing: an
/// answer, so the special is filed nowhere and the map is kept. Any
/// other failed read of it fails the map, which is kept nowhere.
#[tokio::test]
async fn a_related_entry_gone_holds_nothing_and_a_failed_read_fails_the_map() {
    for (status, fails) in [(404u16, false), (500, true)] {
        let kitsu = ubw_kitsu().await;
        lone_special(&kitsu, &sequel_7882(1)).await;
        let anilist_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_string_contains("streamingEpisodes"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&anilist_server)
            .await;
        no_anilist_answer(&anilist_server).await;
        let state = state_with_kitsu(&kitsu.uri());
        let provider = anilist(&anilist_server.uri());
        let tokens = tokens();
        let base = anilist_server.uri();
        let got = episode_map_for_unresolved(
            &ask(&state, ProviderKind::AniList, &provider, &tokens, &base),
            "90001",
        )
        .await;
        let key = map_key(ProviderKind::AniList, "90001", None);
        if fails {
            assert!(got.is_err(), "{status}: {got:?}");
            assert!(
                meta_cache_get(&state.cache_pool, &key)
                    .expect("read")
                    .is_none(),
                "{status}: kept nowhere"
            );
        } else {
            assert_eq!(
                got.expect("the map"),
                EpisodeMap::identity(Identity::Unexplained)
            );
            assert!(meta_cache_get(&state.cache_pool, &key)
                .expect("read")
                .is_some());
        }
    }
}
