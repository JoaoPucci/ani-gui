//! Every URI a manifest carries comes back on the proxy, or not at
//! all: a player or a relayed download reads the rewritten manifest,
//! and a URI left pointing upstream is fetched without the session's
//! referer and outside the host's budget, and a relative one resolves
//! against the proxy and reaches nothing.

use super::*;

fn origin() -> ProxyOrigin {
    ProxyOrigin::new("127.0.0.1", 42_337)
}

/// Every `URI="..."` attribute value in `out`, and every URI line.
fn uris(out: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in out.lines() {
        if line.starts_with('#') {
            let mut rest = line;
            while let Some(at) = rest.find("URI=\"") {
                let after = &rest[at + 5..];
                let end = after.find('"').unwrap_or(after.len());
                found.push(after[..end].to_string());
                rest = &after[end..];
            }
        } else if !line.trim().is_empty() {
            found.push(line.trim().to_string());
        }
    }
    found
}

fn all_proxied(out: &str) -> bool {
    uris(out).iter().all(|u| u.starts_with(&origin().base))
}

#[test]
fn a_masters_session_key_and_session_data_come_back_on_the_proxy() {
    let body = b"#EXTM3U\n\
        #EXT-X-VERSION:5\n\
        #EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"keys/session.key\"\n\
        #EXT-X-SESSION-DATA:DATA-ID=\"com.example.lyrics\",URI=\"data/lyrics.json\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
        v/index.m3u8\n";
    let master = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
    let out = rewrite_master(
        body,
        &master,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite");
    assert!(out.contains("#EXT-X-SESSION-KEY"), "kept: {out}");
    assert!(!out.contains("keys/session.key"), "{out}");
    assert!(!out.contains("data/lyrics.json"), "{out}");
    assert!(all_proxied(&out), "{out}");
}

#[test]
fn a_masters_tags_the_rewrite_cannot_follow_are_dropped() {
    let body = b"#EXTM3U\n\
        #EXT-X-CONTENT-STEERING:SERVER-URI=\"https://steer.example/s\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
        v/index.m3u8\n";
    let master = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
    let out = rewrite_master(
        body,
        &master,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite");
    assert!(!out.contains("steer.example"), "{out}");
    assert!(all_proxied(&out), "{out}");
}

#[test]
fn a_media_playlists_low_latency_hints_and_interstitials_are_dropped() {
    let body = b"#EXTM3U\n\
        #EXT-X-VERSION:9\n\
        #EXT-X-TARGETDURATION:4\n\
        #EXT-X-PART-INF:PART-TARGET=1.0\n\
        #EXT-X-DATERANGE:ID=\"ad\",START-DATE=\"2026-01-01T00:00:00Z\",X-ASSET-URI=\"https://ads.example/a.m3u8\"\n\
        #EXT-X-PART:DURATION=1.0,URI=\"part0.m4s\"\n\
        #EXTINF:4.0,\n\
        seg0.m4s\n\
        #EXT-X-PRELOAD-HINT:TYPE=PART,URI=\"part1.m4s\"\n\
        #EXT-X-RENDITION-REPORT:URI=\"../720/index.m3u8\",LAST-MSN=1\n\
        #EXTINF:4.0,\n\
        seg1.m4s\n";
    let media = Url::parse("https://cdn.example/a/1080/index.m3u8").expect("url");
    let out = rewrite_media(
        body,
        &media,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite");
    for upstream in ["part0.m4s", "part1.m4s", "720/index.m3u8", "ads.example"] {
        assert!(!out.contains(upstream), "{upstream} left in {out}");
    }
    assert!(all_proxied(&out), "{out}");
}

proptest::proptest! {
    /// Whatever URI-bearing tags a master carries, every URI in the
    /// rewrite is on the proxy.
    #[test]
    fn every_uri_a_master_carries_comes_back_on_the_proxy(
        session_keys in 0usize..3,
        session_data in 0usize..3,
        valued_data in proptest::bool::ANY,
        unkeyed in proptest::bool::ANY,
        renditions in 0usize..3,
        uriless_rendition in proptest::bool::ANY,
        steering in proptest::bool::ANY,
        iframes in 0usize..3,
        variants in 1usize..4,
    ) {
        let mut body = String::from("#EXTM3U\n#EXT-X-VERSION:6\n");
        if steering {
            body.push_str("#EXT-X-CONTENT-STEERING:SERVER-URI=\"https://steer.example/s\"\n");
        }
        for i in 0..session_keys {
            body.push_str(&format!("#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"k/{i}.key\"\n"));
        }
        if unkeyed {
            body.push_str("#EXT-X-SESSION-KEY:METHOD=NONE\n");
        }
        for i in 0..session_data {
            body.push_str(&format!("#EXT-X-SESSION-DATA:DATA-ID=\"d{i}\",URI=\"d/{i}.json\"\n"));
        }
        if valued_data {
            body.push_str("#EXT-X-SESSION-DATA:DATA-ID=\"v\",VALUE=\"kept\"\n");
        }
        if uriless_rendition {
            body.push_str("#EXT-X-MEDIA:TYPE=CLOSED-CAPTIONS,GROUP-ID=\"cc\",NAME=\"cc\",INSTREAM-ID=\"CC1\"\n");
        }
        for i in 0..renditions {
            body.push_str(&format!(
                "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"n{i}\",URI=\"a/{i}.m3u8\"\n"
            ));
        }
        for i in 0..iframes {
            body.push_str(&format!(
                "#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=1000,URI=\"if/{i}.m3u8\"\n"
            ));
        }
        for i in 0..variants {
            body.push_str(&format!("#EXT-X-STREAM-INF:BANDWIDTH={}\nv/{i}.m3u8\n", 1000 + i));
        }
        let master = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
        let out = rewrite_master(body.as_bytes(), &master, &origin(), SessionId::new(), &AppSecret::random())
            .expect("rewrite");
        proptest::prop_assert!(all_proxied(&out), "{}", out);
        // Rewritten, not dropped: every session key and session data
        // the master carried is still there, each URI resolved
        // against the master.
        let keys = session_keys + usize::from(unkeyed);
        proptest::prop_assert_eq!(out.matches("#EXT-X-SESSION-KEY").count(), keys);
        let data = session_data + usize::from(valued_data);
        proptest::prop_assert_eq!(out.matches("#EXT-X-SESSION-DATA").count(), data);
        for i in 0..session_keys {
            let upstream = format!("https://cdn.example/a/k/{i}.key");
            let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(upstream.as_bytes());
            proptest::prop_assert!(out.contains(&encoded), "{} not resolved in {}", upstream, out);
        }
    }

    /// Whatever URI-bearing tags a media playlist carries, every URI in
    /// the rewrite is on the proxy.
    #[test]
    fn every_uri_a_media_playlist_carries_comes_back_on_the_proxy(
        segments in 1usize..5,
        keyed in proptest::bool::ANY,
        mapped in proptest::bool::ANY,
        parts in proptest::bool::ANY,
    ) {
        let mut body = String::from("#EXTM3U\n#EXT-X-VERSION:9\n#EXT-X-TARGETDURATION:4\n");
        if mapped {
            body.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n");
        }
        for i in 0..segments {
            if keyed {
                body.push_str(&format!("#EXT-X-KEY:METHOD=AES-128,URI=\"k/{i}.key\"\n"));
            }
            if parts {
                body.push_str(&format!("#EXT-X-PART:DURATION=1.0,URI=\"p{i}.m4s\"\n"));
            }
            body.push_str(&format!("#EXTINF:4.0,\ns{i}.m4s\n"));
        }
        // Hints and reports ahead of a segment: the parser drops tags
        // after the last one, so only these reach the rewrite.
        if parts {
            body.push_str("#EXT-X-PRELOAD-HINT:TYPE=PART,URI=\"next.m4s\"\n");
            body.push_str("#EXT-X-RENDITION-REPORT:URI=\"../b/index.m3u8\",LAST-MSN=1\n");
            body.push_str("#EXTINF:4.0,\nlast.m4s\n");
        }
        let media = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
        let out = rewrite_media(body.as_bytes(), &media, &origin(), SessionId::new(), &AppSecret::random())
            .expect("rewrite");
        proptest::prop_assert!(all_proxied(&out), "{}", out);
    }
}
