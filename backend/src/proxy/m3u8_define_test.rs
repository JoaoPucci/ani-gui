//! `EXT-X-DEFINE` variables are substituted before the rewrite reads
//! a URI, so a URI built from them comes back on the proxy as the URI
//! the variables spell, not as the literal `{$name}` text.

use super::*;
use std::collections::HashMap;

fn origin() -> ProxyOrigin {
    ProxyOrigin::new("127.0.0.1", 42_337)
}

/// The upstream URL a proxied URI in `out` carries, for every URI line.
fn upstreams(out: &str) -> Vec<String> {
    out.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let u = l
                .split("u=")
                .nth(1)
                .expect("proxied")
                .split('&')
                .next()
                .expect("u");
            String::from_utf8(
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(u)
                    .expect("base64"),
            )
            .expect("utf8")
        })
        .collect()
}

#[test]
fn a_masters_variables_spell_its_uris() {
    let body = b"#EXTM3U\n\
        #EXT-X-VERSION:11\n\
        #EXT-X-DEFINE:NAME=\"edge\",VALUE=\"https://edge.example/x\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
        {$edge}/1080/index.m3u8\n";
    let master = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
    let out = rewrite_master(
        body,
        &master,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite");
    assert!(!out.contains("{$"), "{out}");
    assert_eq!(upstreams(&out), ["https://edge.example/x/1080/index.m3u8"]);
}

#[test]
fn a_query_parameter_of_the_playlists_own_url_is_a_variable() {
    let body = b"#EXTM3U\n\
        #EXT-X-VERSION:11\n\
        #EXT-X-DEFINE:QUERYPARAM=\"token\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
        v.m3u8?token={$token}\n";
    let master = Url::parse("https://cdn.example/a/master.m3u8?token=abc").expect("url");
    let out = rewrite_master(
        body,
        &master,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite");
    assert_eq!(upstreams(&out), ["https://cdn.example/a/v.m3u8?token=abc"]);
}

#[test]
fn a_media_playlist_imports_what_its_master_defined() {
    let body = b"#EXTM3U\n\
        #EXT-X-VERSION:11\n\
        #EXT-X-DEFINE:IMPORT=\"edge\"\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXTINF:5.0,\n\
        {$edge}/s0.ts\n\
        #EXT-X-ENDLIST\n";
    let media = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    let imported = HashMap::from([("edge".to_string(), "https://edge.example/x".to_string())]);
    let out = rewrite_media_importing(
        body,
        &media,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
        Stream::Main,
        &imported,
    )
    .expect("rewrite");
    assert!(!out.contains("{$"), "{out}");
    assert_eq!(upstreams(&out), ["https://edge.example/x/s0.ts"]);
}

#[test]
fn a_masters_variables_are_what_its_media_playlists_import() {
    let body = b"#EXTM3U\n\
        #EXT-X-DEFINE:NAME=\"edge\",VALUE=\"https://edge.example/x\"\n\
        #EXT-X-DEFINE:QUERYPARAM=\"token\"\n\
        #EXT-X-STREAM-INF:BANDWIDTH=1\n\
        v.m3u8\n";
    let master = Url::parse("https://cdn.example/a/master.m3u8?token=abc").expect("url");
    let vars = master_variables(body, &master);
    assert_eq!(
        vars.get("edge").map(String::as_str),
        Some("https://edge.example/x")
    );
    assert_eq!(vars.get("token").map(String::as_str), Some("abc"));
}

#[test]
fn a_reference_to_nothing_defined_is_left_as_written() {
    // Not a variable the playlist defines: there is nothing to spell,
    // and the playlist is malformed rather than the rewrite's to fix.
    let body = b"#EXTM3U\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXTINF:5.0,\n\
        s{$missing}.ts\n\
        #EXT-X-ENDLIST\n";
    let media = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    let out = rewrite_media(
        body,
        &media,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
    )
    .expect("rewrite");
    assert_eq!(
        upstreams(&out),
        ["https://cdn.example/a/s%7B$missing%7D.ts"]
    );
}

proptest::proptest! {
    /// Whatever variables a playlist defines, by name and value or by
    /// a query parameter of its URL, every reference to them is
    /// spelled out and no definition is left; the playlist's other
    /// lines are as they were.
    #[test]
    fn every_defined_reference_is_spelled_out(
        vars in proptest::collection::hash_map("[a-z][a-z0-9_]{0,6}", "[A-Za-z0-9./:_-]{0,12}", 0..5),
        by_query in proptest::bool::ANY,
    ) {
        let mut url = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
        let mut body = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:5\n");
        for (name, value) in &vars {
            if by_query {
                url.query_pairs_mut().append_pair(name, value);
                body.push_str(&format!("#EXT-X-DEFINE:QUERYPARAM=\"{name}\"\n"));
            } else {
                body.push_str(&format!("#EXT-X-DEFINE:NAME=\"{name}\",VALUE=\"{value}\"\n"));
            }
        }
        let mut expected = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:5\n");
        for (i, (name, value)) in vars.iter().enumerate() {
            body.push_str(&format!("#EXTINF:5.0,\ns{i}/{{${name}}}.ts\n"));
            expected.push_str(&format!("#EXTINF:5.0,\ns{i}/{value}.ts\n"));
        }
        let out = super::super::m3u8_define::substitute(body.as_bytes(), &url, &HashMap::new());
        let out = String::from_utf8(out).expect("utf8");
        proptest::prop_assert_eq!(out, expected);
    }
}
