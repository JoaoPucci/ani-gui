//! The tracks the backend orders — an external player's, a download's,
//! a cut listing's, the ones a cached row's check reads — follow the
//! locale the renderer runs in, which the built-in player starts by,
//! not the raw `locale` in `config.toml`. The renderer adopts the
//! configured locale only when it is one of the app's own; a
//! hand-edited `zhO-` or `zh_TW` (both of which `Intl.Locale` rejects)
//! leaves it on another, and the two orders would then start on
//! different tracks.

use crate::commands::play::tests::{cached_walked, state_with_proxy_origin, track};
use crate::commands::play::PlayArgs;
use crate::commands::play_resolution_cache::CachedResolution;
use crate::proxy::MediaKind;

/// Configured locales the renderer does not adopt. The backend's own
/// reader takes both for Chinese, so each pulls the order away from
/// any renderer locale but Chinese.
const HAND_EDITED: [&str; 2] = ["zhO-", "zh_TW"];

fn play_args(locale: &str) -> PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Show",
        "episode": "1",
        "mode": "sub",
        "locale": locale,
    }))
    .expect("args")
}

fn config(locale: &str) -> crate::config::Config {
    crate::config::Config {
        locale: locale.into(),
        cache_resolutions: true,
        ..Default::default()
    }
}

/// A listing with a track in each language either locale could name,
/// the provider's default among none of them.
fn listing() -> CachedResolution {
    let mut cached = cached_walked(
        "https://cdn.example/master.m3u8".into(),
        String::new(),
        MediaKind::Hls,
    );
    cached.subtitles = ["zh-Hans", "pt-BR", "es-419", "en"]
        .iter()
        .enumerate()
        .map(|(i, lang)| {
            track(
                lang,
                *lang == "es-419",
                &format!("https://cdn.example/{i}.vtt"),
            )
        })
        .collect();
    cached
}

/// The track the built-in player starts on for this listing in each
/// renderer locale: `startingTrack` in `$lib/play/caption-start.ts`,
/// which `api-locale.test.ts` asserts on the same listing. A renderer
/// whose locale Paraglide cannot resolve sends an empty one, which
/// names no language, so the player starts on the provider's default.
const STARTS: [(&str, usize); 3] = [("en", 3), ("pt-BR", 1), ("", 2)];

#[test]
fn an_external_player_starts_on_the_renderers_locale_not_a_hand_edited_one() {
    for configured in HAND_EDITED {
        for (renderer, start) in STARTS {
            let launch =
                super::cached_launch_args(listing(), &play_args(renderer), &config(configured));
            assert_eq!(
                launch.subtitle_urls.first().map(String::as_str),
                Some(format!("https://cdn.example/{start}.vtt").as_str()),
                "config {configured:?}, renderer {renderer:?}"
            );
            assert!(!launch.subtitles_off);
        }
    }
}

#[test]
fn a_resolve_asks_for_the_renderers_locale() {
    for configured in HAND_EDITED {
        let args = play_args("es-419");
        assert_eq!(args.wanted(configured).locale, "es-419");
        assert_eq!(
            args.native_request("best", configured, None, None)
                .wanted
                .locale,
            "es-419"
        );
    }
}

/// A single episode's download and a range's both order by
/// [`DownloadArgs::ordering_locale`](crate::commands::download::DownloadArgs::ordering_locale).
#[test]
fn a_download_orders_by_the_renderers_locale() {
    let download = |json: serde_json::Value| -> crate::commands::download::DownloadArgs {
        serde_json::from_value(json).expect("args")
    };
    for configured in HAND_EDITED {
        let td = tempfile::tempdir().expect("td");
        let path = td.path().join("config.toml");
        std::fs::write(&path, format!("locale = \"{configured}\"\n")).expect("config");
        let named = download(serde_json::json!({
            "title": "Show", "episode": "1-3", "mode": "sub", "locale": "es-419",
        }));
        assert_eq!(named.ordering_locale(&path), "es-419");
        let unnamed = download(serde_json::json!({
            "title": "Show", "episode": "1-3", "mode": "sub",
        }));
        assert_eq!(unnamed.ordering_locale(&path), configured);
    }
}

/// A request that names no locale — an older renderer, a direct API
/// caller — orders by the configured one, as before.
#[test]
fn a_request_without_a_locale_orders_by_the_configured_one() {
    let args: PlayArgs = serde_json::from_value(
        serde_json::json!({ "title": "Show", "episode": "1", "mode": "sub" }),
    )
    .expect("args");
    assert_eq!(args.wanted("pt-BR").locale, "pt-BR");
}

/// The check a cached row passes before a replay reads the tracks the
/// player would start on: in the renderer's locale, so a dead track in
/// it keeps the row from being served even when the configured locale
/// names another language.
#[tokio::test]
async fn a_cached_rows_check_reads_the_renderers_locale() {
    use wiremock::matchers::{method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/subs/11.vtt"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/subs/\d+\.vtt$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"WEBVTT\n\n".to_vec()))
        .mount(&server)
        .await;
    // Every other track is in a known language too, so only the
    // renderer's locale can lift the last one into the check.
    const OTHERS: [&str; 11] = [
        "ar", "de", "fr", "it", "ja", "ko", "nl", "pl", "ru", "sv", "tr",
    ];
    for configured in HAND_EDITED {
        let td = tempfile::tempdir().expect("td");
        let mut state = state_with_proxy_origin();
        state.config_path = td.path().join("config.toml");
        state.history_path = td.path().join("history");
        state.state_dir = td.path().join("state");
        std::fs::write(
            &state.config_path,
            format!("cache_resolutions = true\nlocale = \"{configured}\"\n"),
        )
        .expect("config");
        let mut cached = cached_walked(
            format!("{}/master.m3u8", server.uri()),
            String::new(),
            MediaKind::Hls,
        );
        cached.subtitles = (0..12)
            .map(|i| {
                let lang = if i == 11 { "pt-BR" } else { OTHERS[i] };
                track(lang, i == 0, &format!("{}/subs/{i}.vtt", server.uri()))
            })
            .collect();
        let args = play_args("pt-BR");
        crate::commands::play_resolution_cache::put(
            &state.cache_pool,
            &args.resolution_key(),
            &cached,
        );
        assert!(
            super::try_launch_args_from_cache(&state, &args, &config(configured))
                .await
                .is_none(),
            "config {configured:?}: the renderer's track is gone, so the handoff resolves afresh"
        );
        crate::commands::play_resolution_cache::put(
            &state.cache_pool,
            &args.resolution_key(),
            &cached,
        );
        let played = crate::commands::play::play_with_progress(&state, &args, |_| {}).await;
        assert!(
            !played.as_ref().is_ok_and(|r| r.cache_hit),
            "config {configured:?}: the renderer's track is gone, so the play resolves afresh"
        );
    }
}
