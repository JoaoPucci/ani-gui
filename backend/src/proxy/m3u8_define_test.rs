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
            // A query parameter is percent-decoded back to its value.
            let spelled = value.clone();
            expected.push_str(&format!("#EXTINF:5.0,\ns{i}/{spelled}.ts\n"));
        }
        let out = super::super::m3u8_define::substitute(body.as_bytes(), &url, &HashMap::new());
        let out = String::from_utf8(out).expect("utf8");
        proptest::prop_assert_eq!(out, expected);
    }
}

/// A variable's value is inserted as it is, never read again for
/// references: `{$b}` inside `a`'s value stays as written, whatever
/// order the variables are held in.
#[test]
fn a_value_is_not_substituted_again() {
    let body = b"#EXTM3U\n\
        #EXT-X-DEFINE:NAME=\"a\",VALUE=\"x{$b}\"\n\
        #EXT-X-DEFINE:NAME=\"b\",VALUE=\"Y\"\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXTINF:5.0,\n\
        {$a}.ts\n";
    let url = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    for _ in 0..64 {
        let out = super::super::m3u8_define::substitute(body, &url, &HashMap::new());
        let out = String::from_utf8(out).expect("utf8");
        assert!(out.contains("\nx{$b}.ts\n"), "{out}");
    }
}

/// References are spelled out where a playlist may carry them — URI
/// lines and quoted attribute values — and nowhere else: a segment's
/// title is text, not a reference.
#[test]
fn only_uri_lines_and_quoted_values_are_substituted() {
    let body = b"#EXTM3U\n\
        #EXT-X-DEFINE:NAME=\"a\",VALUE=\"A\"\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXT-X-KEY:METHOD=AES-128,URI=\"k/{$a}.key\"\n\
        #EXTINF:5.0,{$a}\n\
        s/{$a}.ts\n";
    let url = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    let out = super::super::m3u8_define::substitute(body, &url, &HashMap::new());
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("URI=\"k/A.key\""), "{out}");
    assert!(out.contains("#EXTINF:5.0,{$a}\n"), "{out}");
    assert!(out.contains("\ns/A.ts\n"), "{out}");
}

/// A query parameter's value is percent-decoded before it goes in, as
/// the spec has it — `%2B` is a `+` and `%26` an `&` — but not
/// form-decoded: a `+` stays a `+`, not a space.
#[test]
fn a_query_parameter_is_percent_decoded_not_form_decoded() {
    let body = b"#EXTM3U\n\
        #EXT-X-DEFINE:QUERYPARAM=\"t\"\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXTINF:5.0,\n\
        s.ts?t={$t}\n";
    let url = Url::parse("https://cdn.example/a/index.m3u8?t=a%2Bb+c%26d").expect("url");
    let out = super::super::m3u8_define::substitute(body, &url, &HashMap::new());
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("\ns.ts?t=a+b+c&d\n"), "{out}");
}

/// Hexadecimal-sequence and enumerated values carry references too, as
/// a quoted value does; a segment's title still does not.
#[test]
fn unquoted_attribute_values_are_substituted() {
    let body = b"#EXTM3U\n\
        #EXT-X-DEFINE:NAME=\"iv\",VALUE=\"00ff\"\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXT-X-KEY:METHOD=AES-128,URI=\"k.key\",IV=0x{$iv}\n\
        #EXTINF:5.0,{$iv}\n\
        s.ts\n";
    let url = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    let out = super::super::m3u8_define::substitute(body, &url, &HashMap::new());
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("IV=0x00ff\n"), "{out}");
    assert!(out.contains("#EXTINF:5.0,{$iv}\n"), "{out}");
}

/// A reference ahead of the definition it names is not spelled: the
/// spec has the definition come first.
#[test]
fn a_reference_before_its_definition_is_left_as_written() {
    let body = b"#EXTM3U\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXTINF:5.0,\n\
        {$a}/s0.ts\n\
        #EXT-X-DEFINE:NAME=\"a\",VALUE=\"A\"\n\
        #EXTINF:5.0,\n\
        {$a}/s1.ts\n";
    let url = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    let out = super::super::m3u8_define::substitute(body, &url, &HashMap::new());
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("\n{$a}/s0.ts\n"), "{out}");
    assert!(out.contains("\nA/s1.ts\n"), "{out}");
}

/// A definition that defined nothing — an import the master does not
/// carry, a query parameter the URL lacks — stays in the playlist, so
/// the player sees the playlist as the host wrote it rather than one
/// that hides the reference it cannot fill.
#[test]
fn a_definition_that_defined_nothing_stays() {
    let body = b"#EXTM3U\n\
        #EXT-X-DEFINE:IMPORT=\"edge\"\n\
        #EXT-X-DEFINE:NAME=\"a\",VALUE=\"A\"\n\
        #EXT-X-TARGETDURATION:5\n\
        #EXTINF:5.0,\n\
        {$edge}/{$a}.ts\n";
    let url = Url::parse("https://cdn.example/a/index.m3u8").expect("url");
    let out = super::super::m3u8_define::substitute(body, &url, &HashMap::new());
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("#EXT-X-DEFINE:IMPORT=\"edge\"\n"), "{out}");
    assert!(!out.contains("NAME=\"a\""), "{out}");
    assert!(out.contains("\n{$edge}/A.ts\n"), "{out}");
}

/// A session's master variables leave with the session, and none are
/// kept for a session that is not there.
#[test]
fn a_sessions_master_variables_leave_with_it() {
    let table = crate::proxy::SessionTable::new();
    let session = crate::proxy::StreamSession::new(
        Url::parse("https://cdn.example/a/master.m3u8").expect("url"),
        String::new(),
    );
    let id = table.insert(session);
    let vars = HashMap::from([("a".to_string(), "A".to_string())]);
    table.set_master_variables(id, vars.clone());
    assert_eq!(table.master_variables(&id), vars);
    table.remove(&id);
    assert!(table.master_variables(&id).is_empty());
    table.set_master_variables(id, vars);
    assert!(
        table.master_variables(&id).is_empty(),
        "nothing kept for a session that is gone"
    );
}

proptest::proptest! {
    /// Whatever a query parameter's value, percent-encoded into the
    /// playlist's URL and read back as a variable, it comes back as it
    /// was: `+` included, which form-decoding would have turned into a
    /// space.
    #[test]
    fn a_query_parameters_value_comes_back_as_it_was(value in "[ -~]{0,16}") {
        let encoded: String = value
            .bytes()
            .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect();
        let url = Url::parse(&format!("https://cdn.example/a/index.m3u8?v={encoded}")).expect("url");
        let vars = super::super::m3u8_define::defined(
            "#EXT-X-DEFINE:QUERYPARAM=\"v\"\n",
            &url,
            &HashMap::new(),
        );
        proptest::prop_assert_eq!(vars.get("v").map(String::as_str), Some(value.as_str()));
    }
}
