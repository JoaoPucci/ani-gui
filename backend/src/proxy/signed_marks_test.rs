//! What a rewritten URI says about what it points at — playlist or
//! media, the stream it carries or feeds, the playback it buys, the
//! variables its master defined — is signed with it: the proxy acts on
//! those marks, so a URI whose marks were changed is refused like one
//! whose upstream was.

use super::*;
use tower::ServiceExt as _;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn proxy_over(master: &str) -> (Router, SessionId) {
    let sessions = SessionTable::new();
    let session = StreamSession::new_with_kind(
        url::Url::parse(master).expect("master url"),
        MediaKind::Hls,
        String::new(),
    );
    let id = session.id;
    sessions.insert(session);
    let state = ProxyState {
        sessions,
        secret: AppSecret::from_bytes([9u8; 32]),
        client: upstream::build_client().expect("client builds"),
        origin: ProxyOrigin::new("127.0.0.1", 1),
        host_budget: host_budget::HostBudget::fresh(),
    };
    (build_router(state), id)
}

async fn fetch(router: &Router, uri: &str) -> (StatusCode, String) {
    let resp = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// The path-and-query a rewritten manifest's line points the player at.
fn proxied(line: &str) -> String {
    line.trim()
        .strip_prefix("http://127.0.0.1:1")
        .expect("a proxied uri")
        .to_owned()
}

/// The upstream a proxied path names.
fn upstream_of(path: &str) -> String {
    let u = url::Url::parse(&format!("http://x{path}")).expect("url");
    let encoded = u
        .query_pairs()
        .find(|(k, _)| k == "u")
        .map(|(_, v)| v.into_owned())
        .expect("u");
    String::from_utf8(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded)
            .expect("base64"),
    )
    .expect("utf8")
}

/// A token signs its marks: the same upstream with other marks, or
/// with none, does not verify.
#[test]
fn a_token_signs_the_marks_it_was_written_with() {
    let secret = AppSecret::from_bytes([3u8; 32]);
    let id = SessionId::new();
    let url = "https://cdn.example/a/s0.ts";
    let token = sign_marked(&secret, id, url, "k=md&r=main&d=5000");
    assert!(verify_marked(&secret, id, url, "k=md&r=main&d=5000", &token).is_ok());
    assert!(verify_marked(&secret, id, url, "k=pl&r=main&d=5000", &token).is_err());
    assert!(verify_marked(&secret, id, url, "k=md&r=main&d=9000", &token).is_err());
    assert!(verify_marked(&secret, id, url, "", &token).is_err());
}

/// A media segment a playlist names is refused once its mark says
/// playlist, or once its marks are dropped — what would otherwise send
/// a segment's whole body through the manifest path.
#[tokio::test]
async fn a_segment_whose_marks_were_changed_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v/index.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-TARGETDURATION:5\n#EXTINF:5.0,\ns0.m3u8\n#EXT-X-ENDLIST\n",
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/v/s0.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"segment".to_vec()))
        .mount(&server)
        .await;
    let master = format!("{}/master.m3u8", server.uri());
    let (router, id) = proxy_over(&master);
    let secret = AppSecret::from_bytes([9u8; 32]);
    let playlist = format!("{}/v/index.m3u8", server.uri());
    let token = sign_marked(&secret, id, &playlist, "k=pl&s=main");
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(playlist.as_bytes());
    let uri = format!(
        "/s/{}/seg?u={encoded}&t={token}&k=pl&s=main",
        id.as_string()
    );
    let (status, body) = fetch(&router, &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let segment = proxied(
        body.lines()
            .find(|l| l.contains("/seg?u="))
            .expect("a segment"),
    );
    assert_eq!(fetch(&router, &segment).await.0, StatusCode::OK);
    let flipped = segment.replace("&k=md", "&k=pl");
    assert_ne!(flipped, segment);
    assert_eq!(fetch(&router, &flipped).await.0, StatusCode::FORBIDDEN);
    let bare = segment.split("&k=").next().expect("prefix").to_owned();
    assert_eq!(fetch(&router, &bare).await.0, StatusCode::FORBIDDEN);
}

/// A media playlist imports the variables of the master response that
/// named it: a master served again with other values, while the first
/// response's playlists are still being fetched, does not change what
/// they import.
#[tokio::test]
async fn a_media_playlist_imports_from_the_master_response_that_named_it() {
    let server = MockServer::start().await;
    let master_with = |edge: &str| {
        format!(
            "#EXTM3U\n#EXT-X-DEFINE:NAME=\"edge\",VALUE=\"{edge}\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n"
        )
    };
    Mock::given(method("GET"))
        .and(wm_path("/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(master_with("one")))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(master_with("two")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/v.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-DEFINE:IMPORT=\"edge\"\n#EXT-X-TARGETDURATION:5\n\
             #EXTINF:5.0,\n{$edge}/s0.ts\n#EXT-X-ENDLIST\n",
        ))
        .mount(&server)
        .await;
    let (router, id) = proxy_over(&format!("{}/master.m3u8", server.uri()));
    let master_uri = format!("/s/{}/master.m3u8", id.as_string());
    let (status, first) = fetch(&router, &master_uri).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let variant = proxied(
        first
            .lines()
            .find(|l| l.contains("/seg?u="))
            .expect("a variant"),
    );
    let (status, second) = fetch(&router, &master_uri).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let (status, media) = fetch(&router, &variant).await;
    assert_eq!(status, StatusCode::OK, "{media}");
    let segment = proxied(
        media
            .lines()
            .find(|l| l.contains("/seg?u="))
            .expect("a segment"),
    );
    assert_eq!(upstream_of(&segment), format!("{}/one/s0.ts", server.uri()));
}
