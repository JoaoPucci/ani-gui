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

/// A mark's value cannot carry another mark: separators encoded into a
/// value read back as that value, not as the pairs they spell, so a
/// token signed for a playlist's `k=pl&s=audio` does not verify for a
/// single `k` whose value is `pl&s=audio`.
#[tokio::test]
async fn a_separator_encoded_into_a_mark_does_not_read_as_another_mark() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/a/en.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-TARGETDURATION:5\n#EXTINF:5.0,\nseg.aac\n#EXT-X-ENDLIST\n",
        ))
        .mount(&server)
        .await;
    let (router, id) = proxy_over(&format!("{}/master.m3u8", server.uri()));
    let secret = AppSecret::from_bytes([9u8; 32]);
    let playlist = format!("{}/a/en.m3u8", server.uri());
    let token = sign_marked(&secret, id, &playlist, "k=pl&s=audio");
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(playlist.as_bytes());
    let base = format!("/s/{}/seg?u={encoded}&t={token}", id.as_string());
    assert_eq!(
        fetch(&router, &format!("{base}&k=pl&s=audio")).await.0,
        StatusCode::OK
    );
    assert_eq!(
        fetch(&router, &format!("{base}&k=pl%26s%3Daudio")).await.0,
        StatusCode::FORBIDDEN
    );
}

/// The query of a proxied URI, as the segment route reads it.
fn query_of(uri: &str) -> SegmentQuery {
    let query = uri.split_once('?').expect("a query").1;
    serde_urlencoded::from_str(query).expect("the route reads it")
}

proptest::proptest! {
    /// Whatever variables a master defines, the session keeps them
    /// under the key a playlist's URI names, and gives them back as
    /// they were; a key it does not keep names nothing.
    #[test]
    fn a_masters_variables_come_back_under_their_key(
        vars in proptest::collection::hash_map("[A-Za-z0-9_-]{1,8}", "[ -~]{0,16}", 1..5),
    ) {
        let table = SessionTable::new();
        let session = StreamSession::new(url::Url::parse("https://cdn.example/m.m3u8").expect("url"), String::new());
        let id = table.insert(session);
        let key = table.keep_master_variables(id, vars.clone()).expect("a key");
        proptest::prop_assert_eq!(table.master_variables(&id, &key), Some(vars));
        proptest::prop_assert_eq!(table.master_variables(&id, "0000"), None);
    }

    /// Every URI the rewrite writes — a master's playlists, with and
    /// without variables, and a media playlist's segments, keys and
    /// init segments — reads back on the segment route as the marks its
    /// token signed: the route verifies what the rewrite wrote.
    #[test]
    fn every_uri_the_rewrite_writes_verifies_as_written(
        millis in 0u32..20_000,
        edge in "[a-z]{0,8}",
        audio in proptest::bool::ANY,
    ) {
        let secret = AppSecret::from_bytes([5u8; 32]);
        let id = SessionId::new();
        let origin = ProxyOrigin::new("127.0.0.1", 1);
        let define = if edge.is_empty() { String::new() } else { format!("#EXT-X-DEFINE:NAME=\"edge\",VALUE=\"{edge}\"\n") };
        let master = format!(
            "#EXTM3U\n{define}#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"en\",URI=\"a.m3u8\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1,AUDIO=\"a\"\nv.m3u8\n"
        );
        let master_url = url::Url::parse("https://cdn.example/x/master.m3u8").expect("url");
        let out = rewrite_master(master.as_bytes(), &master_url, &origin, id, &secret).expect("master");
        let seconds = f64::from(millis) / 1000.0;
        let media = format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:20\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.key\"\n\
             #EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:{seconds:.3},\ns0.ts\n#EXT-X-ENDLIST\n"
        );
        let media_url = url::Url::parse("https://cdn.example/x/v.m3u8").expect("url");
        let stream = if audio { Stream::Audio } else { Stream::Main };
        let rewritten = rewrite_media_as(media.as_bytes(), &media_url, &origin, id, &secret, stream).expect("media");
        let uris: Vec<String> = out
            .lines()
            .chain(rewritten.lines())
            .flat_map(|l| l.split('"'))
            .filter(|s| s.contains("/seg?u="))
            .map(|s| s.trim().to_owned())
            .collect();
        proptest::prop_assert!(uris.len() >= 5, "{out}\n{rewritten}");
        for uri in uris {
            let q = query_of(&uri);
            let upstream = decode_seg_url(&q.u).expect("upstream");
            proptest::prop_assert!(
                verify_marked(&secret, id, upstream.as_str(), &marks_of(&q), &q.t).is_ok(),
                "{uri}"
            );
        }
    }
}
/// A playlist URI whose variables do not read as variables is refused
/// with a 400, though its token is good — before the playlist is
/// fetched, so the host is not asked for what cannot be rewritten.
#[tokio::test]
async fn a_playlist_uri_whose_variables_do_not_read_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wm_path("/v.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
        .expect(0)
        .mount(&server)
        .await;
    let (router, id) = proxy_over(&format!("{}/master.m3u8", server.uri()));
    let secret = AppSecret::from_bytes([9u8; 32]);
    let playlist = format!("{}/v.m3u8", server.uri());
    let marks = "k=pl&s=main&v=bm90LWpzb24";
    let token = sign_marked(&secret, id, &playlist, marks);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(playlist.as_bytes());
    let uri = format!("/s/{}/seg?u={encoded}&t={token}&{marks}", id.as_string());
    assert_eq!(fetch(&router, &uri).await.0, StatusCode::BAD_REQUEST);
}

/// However much a master defines, the playlist URIs it names stay
/// short: a tool reading the rewritten master — ffmpeg reads each line
/// into a few kilobytes — follows them, and the variables stay with the
/// session under a key the URI names.
#[tokio::test]
async fn a_masters_variables_keep_its_playlist_uris_short() {
    let server = MockServer::start().await;
    let long = "x".repeat(8 * 1024);
    let master = format!(
        "#EXTM3U\n#EXT-X-DEFINE:NAME=\"edge\",VALUE=\"{long}\"\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n"
    );
    Mock::given(method("GET"))
        .and(wm_path("/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(master))
        .mount(&server)
        .await;
    let (router, id) = proxy_over(&format!("{}/master.m3u8", server.uri()));
    let (status, body) = fetch(&router, &format!("/s/{}/master.m3u8", id.as_string())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let variant = body
        .lines()
        .find(|l| l.contains("/seg?u="))
        .expect("a variant");
    assert!(variant.len() < 1024, "{} bytes", variant.len());
}

/// A session forgets its masters' variables with it.
#[test]
fn a_sessions_master_variables_leave_with_it() {
    let table = SessionTable::new();
    let session = StreamSession::new(
        url::Url::parse("https://cdn.example/m.m3u8").expect("url"),
        String::new(),
    );
    let id = table.insert(session);
    let vars = m3u8_define::Variables::from([("a".to_string(), "A".to_string())]);
    let key = table.keep_master_variables(id, vars).expect("a key");
    table.remove(&id);
    assert_eq!(table.master_variables(&id, &key), None);
    assert_eq!(
        table.keep_master_variables(
            id,
            m3u8_define::Variables::from([("a".to_string(), "A".to_string())])
        ),
        None,
        "nothing kept for a session that is gone"
    );
}

proptest::proptest! {
    /// Whatever URI the rewrite writes, folding one of its marks into
    /// the value before it — the separator percent-encoded — is refused:
    /// no mark can be smuggled into or out of another's value.
    #[test]
    fn a_mark_folded_into_another_is_refused(
        millis in 0u32..20_000,
        audio in proptest::bool::ANY,
        pick in 0usize..16,
    ) {
        let secret = AppSecret::from_bytes([5u8; 32]);
        let id = SessionId::new();
        let origin = ProxyOrigin::new("127.0.0.1", 1);
        let seconds = f64::from(millis) / 1000.0;
        let media = format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:20\n#EXTINF:{seconds:.3},\ns0.ts\n#EXT-X-ENDLIST\n"
        );
        let media_url = url::Url::parse("https://cdn.example/x/v.m3u8").expect("url");
        let stream = if audio { Stream::Audio } else { Stream::Main };
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n";
        let master_url = url::Url::parse("https://cdn.example/x/master.m3u8").expect("url");
        let out = rewrite_master_naming(master.as_bytes(), &master_url, &origin, id, &secret, "key0").expect("master")
            + &rewrite_media_as(media.as_bytes(), &media_url, &origin, id, &secret, stream).expect("media");
        let uris: Vec<&str> = out.lines().filter(|l| l.contains("/seg?u=")).collect();
        let uri = uris[pick % uris.len()];
        let (head, marks) = uri.split_once("&k=").expect("marked");
        let marks = format!("k={marks}");
        let joints: Vec<usize> = marks.match_indices('&').map(|(i, _)| i).collect();
        proptest::prop_assume!(!joints.is_empty());
        let at = joints[pick % joints.len()];
        let rest = &marks[at + 1..];
        let folded = format!("{}%26{}", &marks[..at], rest.replacen('=', "%3D", 1));
        let q = query_of(&format!("{head}&{folded}"));
        let upstream = decode_seg_url(&q.u).expect("upstream");
        proptest::prop_assert!(
            verify_marked(&secret, id, upstream.as_str(), &marks_of(&q), &q.t).is_err(),
            "{folded}"
        );
    }
}
