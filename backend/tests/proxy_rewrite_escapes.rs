//! Every playlist the stream proxy serves names nothing the player
//! would fetch past it. Driven through the real axum router on a
//! loopback port, against a wiremock upstream: a URI the rewrite leaves
//! as written is one the renderer fetches without the session's
//! referer and outside the host's budget.

use ani_gui::proxy::{
    bind_loopback, build_router, AppSecret, ProxyOrigin, ProxyState, SessionId, SessionTable,
    StreamSession,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Harness {
    mock: MockServer,
    proxy_port: u16,
    sessions: SessionTable,
}

async fn start_harness() -> Harness {
    let mock = MockServer::start().await;
    let sessions = SessionTable::new();
    let (addr, listener) = bind_loopback(0).await.expect("bind");
    let state = ProxyState {
        sessions: sessions.clone(),
        secret: AppSecret::random(),
        client: ani_gui::proxy::upstream::build_client().expect("client"),
        origin: ProxyOrigin::new("127.0.0.1", addr.port()),
        host_budget: ani_gui::proxy::host_budget::HostBudget::fresh(),
    };
    let router = build_router(state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Harness {
        mock,
        proxy_port: addr.port(),
        sessions,
    }
}

impl Harness {
    fn session(&self, master_path: &str) -> SessionId {
        let upstream = Url::parse(&format!("{}{master_path}", self.mock.uri())).expect("url");
        let sess = StreamSession::new(upstream, "https://embed.example");
        let id = sess.id;
        self.sessions.insert(sess);
        id
    }

    async fn serve(&self, at: &str, body: String) {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&self.mock)
            .await;
    }

    async fn master(&self, session: SessionId) -> String {
        let url = format!(
            "http://127.0.0.1:{}/s/{}/master.m3u8",
            self.proxy_port,
            session.as_string()
        );
        let resp = reqwest::get(&url).await.expect("proxy responds");
        assert_eq!(resp.status(), 200);
        resp.text().await.expect("body")
    }

    /// Whether `uri` is a URL on the proxy's own scheme, host and port.
    fn on_proxy(&self, uri: &str) -> bool {
        Url::parse(uri).is_ok_and(|u| {
            u.scheme() == "http"
                && u.host_str() == Some("127.0.0.1")
                && u.port() == Some(self.proxy_port)
        })
    }
}

/// Every URI the player would read from `out`, as hls.js reads it:
/// lines end at every JavaScript line terminator, and an attribute whose
/// trimmed name ends in `URI`, `URL` or `-LIST` names something to
/// fetch, its value running to the next comma or closing quote.
fn uris(out: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in out
        .split(['\n', '\r', '\u{2028}', '\u{2029}'])
        .filter(|l| !l.trim().is_empty())
    {
        if !line.starts_with('#') {
            found.push(line.trim().to_owned());
            continue;
        }
        for (eq, _) in line.match_indices('=') {
            let name = line[..eq].trim_end().to_ascii_uppercase();
            if !["URI", "URL", "-LIST"].iter().any(|s| name.ends_with(s)) {
                continue;
            }
            let value = line[eq + 1..].trim_start_matches('"');
            found.push(
                value
                    .split(['"', ','])
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
            );
        }
    }
    found
}

#[tokio::test]
async fn a_master_listing_an_inline_playlist_serves_no_inline_playlist() {
    use base64::Engine;
    let h = start_harness().await;
    // The inline playlist names a segment upstream by its absolute URL:
    // left as written, the player would fetch it straight from there.
    let inline = format!(
        "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\n{}/direct/seg0.ts\n#EXT-X-ENDLIST\n",
        h.mock.uri()
    );
    let inline = format!(
        "data:application/vnd.apple.mpegurl;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(inline)
    );
    h.serve(
        "/master.m3u8",
        format!(
            "#EXTM3U\n\
             #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"ja\",URI=\"{inline}\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=2000000,AUDIO=\"aud\"\n\
             {inline}\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
             v/index.m3u8\n"
        ),
    )
    .await;
    let out = h.master(h.session("/master.m3u8")).await;
    assert!(!out.contains("data:"), "{out}");
    let found = uris(&out);
    assert_eq!(found.len(), 1, "{out}");
    assert!(found.iter().all(|u| h.on_proxy(u)), "{out}");
}

/// A variant whose userinfo spells the proxy's own origin is a request
/// to the host after the `@`; the master serves it on the proxy, and
/// the proxied URL reaches that host with the session's referer.
#[tokio::test]
async fn a_variant_dressed_as_the_proxy_by_its_userinfo_is_proxied() {
    use wiremock::matchers::header;
    let h = start_harness().await;
    let upstream = Url::parse(&h.mock.uri()).expect("mock url");
    let dressed = format!(
        "http://127.0.0.1:{}@127.0.0.1:{}/elsewhere/index.m3u8",
        h.proxy_port,
        upstream.port().expect("mock port")
    );
    h.serve(
        "/master.m3u8",
        format!("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\n{dressed}\n"),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/elsewhere/index.m3u8"))
        .and(header("referer", "https://embed.example"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n",
        ))
        .expect(1)
        .mount(&h.mock)
        .await;
    let out = h.master(h.session("/master.m3u8")).await;
    let found = uris(&out);
    assert_eq!(found.len(), 1, "{out}");
    assert!(h.on_proxy(&found[0]), "{out}");
    let media = reqwest::get(&found[0]).await.expect("variant");
    assert_eq!(media.status(), 200);
    let media = media.text().await.expect("body");
    assert!(uris(&media).iter().all(|u| h.on_proxy(u)), "{media}");
}

/// A source that serves a single-quality stream answers the session's
/// URL with a media playlist rather than a master. The route serves it
/// as the media playlist it is, every segment on the proxy, rather than
/// as a master with no variants, which the player cannot play.
#[tokio::test]
async fn a_media_playlist_served_where_a_master_is_expected_keeps_its_segments() {
    let h = start_harness().await;
    h.serve(
        "/index.m3u8",
        "#EXTM3U\n\
         #EXT-X-VERSION:3\n\
         #EXT-X-TARGETDURATION:6\n\
         #EXT-X-MEDIA-SEQUENCE:0\n\
         #EXTINF:6.0,\n\
         seg0.ts\n\
         #EXTINF:6.0,\n\
         seg1.ts\n\
         #EXT-X-ENDLIST\n"
            .to_owned(),
    )
    .await;
    h.serve("/seg0.ts", "segment zero".to_owned()).await;
    let out = h.master(h.session("/index.m3u8")).await;
    let found = uris(&out);
    assert_eq!(found.len(), 2, "{out}");
    assert!(found.iter().all(|u| h.on_proxy(u)), "{out}");
    assert_eq!(out.matches("#EXTINF:6").count(), 2, "{out}");
    assert!(out.contains("#EXT-X-ENDLIST"), "{out}");
    let segment = reqwest::get(&found[0]).await.expect("segment");
    assert_eq!(segment.status(), 200);
    assert_eq!(segment.text().await.expect("bytes"), "segment zero");
}

/// Text m3u8-rs keeps as written but hls.js reads as a URI — an
/// attribute name with a space before its `=`, a line ended by U+2028 —
/// never reaches the player pointing upstream.
#[tokio::test]
async fn a_master_spelled_for_the_player_alone_names_nothing_upstream() {
    let h = start_harness().await;
    h.serve(
        "/master.m3u8",
        "#EXTM3U\n\
         #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"en\",DEFAULT=YES,URI =\"https://evil.example/a.m3u8\"\n\
         #EXT-X-CONTENT-STEERING:SERVER-URI =\"https://evil.example/steer.json\"\n\
         #EXT-X-FOO:a\u{2028}#EXT-X-STREAM-INF:BANDWIDTH=1\u{2028}https://evil.example/v.m3u8\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1000000,AUDIO=\"aud\"\n\
         v/index.m3u8\n"
            .to_owned(),
    )
    .await;
    let out = h.master(h.session("/master.m3u8")).await;
    assert!(!out.contains("evil.example"), "{out}");
    assert!(uris(&out).iter().all(|u| h.on_proxy(u)), "{out}");
}
