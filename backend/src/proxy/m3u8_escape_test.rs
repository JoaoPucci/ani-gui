//! A playlist entry the rewrite leaves as written is one the player
//! loads itself. An inline `data:` playlist is loaded by the player and
//! parsed like any other, so the absolute upstream URIs inside it are
//! fetched straight from upstream, never seeing the proxy. A master's
//! variant, I-frame variant or rendition that is not a network URI is
//! therefore dropped; only media — a key, an init segment, a session
//! key or session data — may stay inline.

use super::*;

fn origin() -> ProxyOrigin {
    ProxyOrigin::new("127.0.0.1", 42_337)
}

/// An inline media playlist whose one segment sits upstream.
const INLINE_PLAYLIST: &str = "data:application/vnd.apple.mpegurl;base64,\
    I0VYVE0zVQojRVhULVgtVEFSR0VURFVSQVRJT046NAojRVhUSU5GOjQuMCwKaHR0cHM6Ly9jZG4uZXhhbXBsZS92L3NlZzAudHMKI0VYVC1YLUVORExJU1QK";

/// Every `URI="..."` attribute value in `out`, and every URI line.
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

/// Whether `uri` is a URL on the proxy's own scheme, host and port.
fn on_proxy(uri: &str) -> bool {
    Url::parse(uri).is_ok_and(|u| {
        u.scheme() == "http" && u.host_str() == Some("127.0.0.1") && u.port() == Some(42_337)
    })
}

fn master(body: &str) -> String {
    let url = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
    rewrite_master(
        body.as_bytes(),
        &url,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite")
}

#[test]
fn a_variant_written_as_an_inline_playlist_is_dropped() {
    let out = master(&format!(
        "#EXTM3U\n\
         #EXT-X-STREAM-INF:BANDWIDTH=2000000\n\
         {INLINE_PLAYLIST}\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
         v/index.m3u8\n"
    ));
    assert!(!out.contains("data:"), "{out}");
    assert_eq!(uris(&out).len(), 1, "{out}");
    assert!(uris(&out).iter().all(|u| on_proxy(u)), "{out}");
    assert!(out.contains("BANDWIDTH=1000000"), "{out}");
    assert!(!out.contains("BANDWIDTH=2000000"), "{out}");
}

#[test]
fn a_rendition_written_as_an_inline_playlist_is_dropped() {
    let out = master(&format!(
        "#EXTM3U\n\
         #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"ja\",URI=\"{INLINE_PLAYLIST}\"\n\
         #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"en\",URI=\"a/en.m3u8\"\n\
         #EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"sub\",NAME=\"en\",URI=\"{INLINE_PLAYLIST}\"\n\
         #EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=1,URI=\"{INLINE_PLAYLIST}\"\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1000000,AUDIO=\"aud\"\n\
         v/index.m3u8\n"
    ));
    assert!(!out.contains("data:"), "{out}");
    assert!(!out.contains("NAME=\"ja\""), "{out}");
    assert!(out.contains("NAME=\"en\""), "{out}");
    assert!(uris(&out).iter().all(|u| on_proxy(u)), "{out}");
}

/// Media the master names stays inline: a session key written as
/// `data:` is no playlist and names nothing further to fetch.
#[test]
fn a_masters_inline_session_key_stays_beside_a_dropped_inline_variant() {
    let key = "data:text/plain;base64,AAECAwQFBgcICQoLDA0ODw==";
    let out = master(&format!(
        "#EXTM3U\n\
         #EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"{key}\"\n\
         #EXT-X-STREAM-INF:BANDWIDTH=2000000\n\
         {INLINE_PLAYLIST}\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
         v/index.m3u8\n"
    ));
    assert!(out.contains(&format!("URI=\"{key}\"")), "{out}");
    assert!(!out.contains(INLINE_PLAYLIST), "{out}");
}

/// A URI a property can place anywhere a manifest names one: inline,
/// relative, under a scheme of its own, already proxied, or absolute
/// upstream — one of them dressed as the proxy's origin by its userinfo.
fn any_uri() -> impl proptest::strategy::Strategy<Value = String> {
    use proptest::prelude::*;
    let host = prop_oneof![
        Just("127.0.0.1".to_owned()),
        Just("localhost".to_owned()),
        Just("evil.example".to_owned()),
        Just("127.0.0.1:42337@evil.example".to_owned()),
        Just("u:p@127.0.0.1".to_owned()),
        Just("[::1]".to_owned()),
    ];
    prop_oneof![
        "[A-Za-z0-9+/]{0,24}"
            .prop_map(|b| format!("data:application/vnd.apple.mpegurl;base64,{b}")),
        "[a-z]{0,8}".prop_map(|t| format!("data:,{t}")),
        "[a-z]{1,8}(/[a-z]{1,8})?\\.m3u8",
        "(ftp|blob|skd|javascript|file):[a-z]{1,8}",
        Just("http://127.0.0.1:42337/s/x/seg?u=a&t=b".to_owned()),
        ("https?", host, proptest::option::of(1u16..), "[a-z/]{0,10}").prop_map(
            |(scheme, host, port, path)| match port {
                Some(port) => format!("{scheme}://{host}:{port}/{path}"),
                None => format!("{scheme}://{host}/{path}"),
            }
        ),
    ]
}

proptest::proptest! {
    /// Whatever a master names as a playlist, each one in the rewrite
    /// is on the proxy: nothing upstream, nothing inline.
    #[test]
    fn every_playlist_a_master_names_comes_back_on_the_proxy(
        variant in any_uri(),
        audio in any_uri(),
        subtitles in any_uri(),
        iframe in any_uri(),
    ) {
        let url = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
        let body = format!(
            "#EXTM3U\n\
             #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"a\",URI=\"{audio}\"\n\
             #EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"sub\",NAME=\"s\",URI=\"{subtitles}\"\n\
             #EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=1,URI=\"{iframe}\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1000000,AUDIO=\"aud\",SUBTITLES=\"sub\"\n\
             {variant}\n"
        );
        let out = rewrite_master(body.as_bytes(), &url, &origin(), SessionId::new(), &AppSecret::random());
        proptest::prop_assume!(out.is_ok());
        let out = out.expect("rewrite");
        for uri in uris(&out) {
            proptest::prop_assert!(on_proxy(&uri), "{} left in {}", uri, out);
        }
        let escaped = super::player_view::escapes(&out);
        proptest::prop_assert!(escaped.is_empty(), "{:?} read by the player in {}", escaped, out);
    }

    /// Whatever a media playlist names, every network URI in the
    /// rewrite is on the proxy; what stays as written is no fetch.
    #[test]
    fn every_network_uri_a_media_playlist_names_comes_back_on_the_proxy(
        segment in any_uri(),
        key in any_uri(),
        map in any_uri(),
    ) {
        let url = Url::parse("https://cdn.example/a/v.m3u8").expect("url");
        let body = format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n\
             #EXT-X-KEY:METHOD=AES-128,URI=\"{key}\"\n\
             #EXT-X-MAP:URI=\"{map}\"\n\
             #EXTINF:4.0,\n{segment}\n#EXT-X-ENDLIST\n"
        );
        let out = rewrite_media(body.as_bytes(), &url, &origin(), SessionId::new(), &AppSecret::random());
        proptest::prop_assume!(out.is_ok());
        let out = out.expect("rewrite");
        for uri in uris(&out) {
            let network = uri.starts_with("http:") || uri.starts_with("https:");
            proptest::prop_assert!(on_proxy(&uri) || !network, "{} left in {}", uri, out);
        }
        let escaped = super::player_view::escapes(&out);
        proptest::prop_assert!(escaped.is_empty(), "{:?} read by the player in {}", escaped, out);
    }
}
