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

/// Every URI line of `out`, and every `URI="..."` attribute value.
fn uris(out: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        if !line.starts_with('#') {
            found.push(line.trim().to_owned());
            continue;
        }
        for part in line.split("URI=\"").skip(1) {
            found.push(part.split('"').next().unwrap_or_default().to_owned());
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
