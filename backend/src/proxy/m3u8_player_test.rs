//! The rewritten manifest is read by hls.js, not by m3u8-rs, and the
//! two disagree: hls.js trims attribute names (`URI ="…"` is its
//! `URI`), ends lines at U+2028 and U+2029, and calls a playlist media
//! when any line starts `#EXTINF:` or `#EXT-X-TARGETDURATION:`. Text
//! m3u8-rs keeps as opaque — an attribute it did not recognise, a tag
//! it could not parse, a title — must not name anything the player
//! would fetch past the proxy.

use super::player_view::{escapes, reads_as_media, PROXY};
use super::*;

fn origin() -> ProxyOrigin {
    ProxyOrigin::new("127.0.0.1", 42_337)
}

/// What the master route serves for `body`.
fn served(body: &str) -> String {
    let url = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
    rewrite_served(
        body.as_bytes(),
        &url,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
        "",
    )
    .expect("rewrite")
}

fn assert_contained(body: &str) {
    let out = served(body);
    assert!(
        escapes(&out).is_empty(),
        "{:?} escape in\n{out}",
        escapes(&out)
    );
}

#[test]
fn a_rendition_whose_uri_name_is_spaced_names_nothing_upstream() {
    for spaced in ["URI =", "URI\t=", "URI\u{a0}="] {
        assert_contained(&format!(
            "#EXTM3U\n\
             #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"en\",DEFAULT=YES,{spaced}\"https://evil.example/a.m3u8\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1000000,AUDIO=\"aud\"\n\
             v/index.m3u8\n"
        ));
    }
}

#[test]
fn spaced_names_in_a_master_name_nothing_upstream() {
    assert_contained(
        "#EXTM3U\n\
         #EXT-X-CONTENT-STEERING:SERVER-URI =\"https://evil.example/steer.json\",PATHWAY-ID=\"a\"\n\
         #EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=1,URI=\"i.m3u8\",URI =\"https://evil.example/i.m3u8\"\n\
         #EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
         v/index.m3u8\n",
    );
}

#[test]
fn spaced_names_in_a_media_playlist_name_nothing_upstream() {
    assert_contained(
        "#EXTM3U\n#EXT-X-TARGETDURATION:4\n\
         #EXT-X-MAP:URI =\"https://evil.example/init.mp4\"\n\
         #EXT-X-KEY:METHOD=AES-128,KEYFORMATVERSIONS=1,URI =\"https://evil.example/k.bin\"\n\
         #EXT-X-PROGRAM-DATE-TIME:2026-01-01T00:00:00Z\n\
         #EXT-X-DATERANGE:ID=\"i\",CLASS=\"com.apple.hls.interstitial\",START-DATE=\"2026-01-01T00:00:00Z\",X-ASSET-URI =\"https://evil.example/ad.m3u8\"\n\
         #EXT-X-DATERANGE:ID=\"j\",START-DATE=\"2026-01-01T00:00:00Z\",X-ASSET-URL=\"https://evil.example/ad2.m3u8\"\n\
         #EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n",
    );
}

/// A URI only the player resolves upstream: protocol-relative, or
/// spelled by a variable the player substitutes.
#[test]
fn a_kept_attribute_the_player_resolves_upstream_names_nothing() {
    assert_contained(
        "#EXTM3U\n#EXT-X-TARGETDURATION:4\n\
         #EXT-X-MAP:URI=\"init.mp4\",URI =\"//evil.example/init.mp4\"\n\
         #EXT-X-FOO:X-URI =\"{$v}\"\n\
         #EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n",
    );
}

#[test]
fn a_line_ended_by_a_javascript_line_terminator_names_nothing_upstream() {
    for sep in ['\u{2028}', '\u{2029}'] {
        assert_contained(&format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:10\n\
             #EXTINF:10,x{sep}https://evil.example/a.ts\nseg.ts\n#EXT-X-ENDLIST\n"
        ));
        assert_contained(&format!(
            "#EXTM3U\n\
             #EXT-X-FOO:a{sep}#EXTINF:10,{sep}https://evil.example/a.ts\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n"
        ));
    }
}

/// m3u8-rs reads a quoted value up to its closing quote, line breaks
/// and all, and writes it back as it was: a value can carry whole
/// lines of its own into the rewrite — a variant, a segment — which
/// the player reads as lines.
#[test]
fn a_quoted_value_spanning_lines_adds_no_line_upstream() {
    for brk in ["\n", "\r", "\r\n"] {
        assert_contained(&format!(
            "#EXTM3U\n\
             #EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"s\",NAME=\"x{brk}#EXT-X-STREAM-INF:BANDWIDTH=9000000{brk}https://evil.example/v.m3u8{brk}#\"\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1000000\nv/index.m3u8\n"
        ));
        assert_contained(&format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n\
             #EXT-X-DATERANGE:ID=\"d{brk}#EXTINF:4,{brk}https://evil.example/a.ts{brk}#\",START-DATE=\"2026-01-01T00:00:00Z\"\n\
             #EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n"
        ));
    }
}

/// hls.js calls a playlist media by its lines, wherever its first
/// decisive tag sits; the route serves it as the playlist the player
/// will read, its segment on the proxy.
#[test]
fn a_playlist_the_player_reads_as_media_is_rewritten_as_media() {
    let out = served(
        "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n\
         #EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n",
    );
    assert!(reads_as_media(&out), "{out}");
    let segments: Vec<&str> = out
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect();
    assert!(!segments.is_empty(), "{out}");
    assert!(
        segments
            .iter()
            .all(|s| s.starts_with(PROXY) && s.contains("&k=md")),
        "{out}"
    );
}

/// The route tells master from media by what the rewrite parses —
/// after a variable a query parameter defines has been substituted —
/// so a playlist the player reads as media is rewritten as media.
#[test]
fn a_substituted_variable_cannot_make_a_master_read_as_media() {
    let url =
        Url::parse("https://cdn.example/a/master.m3u8?p=%E2%80%A8%23EXT-X-TARGETDURATION%3A4")
            .expect("url");
    let body = "#EXTM3U\n#EXT-X-DEFINE:QUERYPARAM=\"p\"\n\
                #EXT-X-SESSION-DATA:DATA-ID=\"d\",VALUE=\"{$p}\"\n\
                #EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n";
    let out = rewrite_served(
        body.as_bytes(),
        &url,
        &origin(),
        SessionId::new(),
        &AppSecret::random(),
        "",
    )
    .expect("rewrite");
    assert!(!(reads_as_media(&out) && out.contains("&k=pl")), "{out}");
}

/// A tag, an attribute name and a value from the shapes a hostile
/// playlist can take — a quoted value carrying lines of its own among
/// them.
fn any_line() -> impl proptest::strategy::Strategy<Value = String> {
    use proptest::prelude::*;
    let tag = "(EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"n\",|EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=1,|EXT-X-MAP:|EXT-X-KEY:METHOD=AES-128,|EXT-X-KEY:METHOD=AES-128,KEYFORMATVERSIONS=1,|EXT-X-CONTENT-STEERING:|EXT-X-SESSION-DATA:DATA-ID=\"d\",|EXT-X-DATERANGE:ID=\"i\",START-DATE=\"2026-01-01T00:00:00Z\",|EXT-X-FOO:|EXTINF:4,)";
    let name = "(URI|SERVER-URI|X-ASSET-URI|X-ASSET-LIST|X-ASSET-URL)";
    let gap = "( |\t|\u{a0}|)";
    let value = "(https://evil\\.example/x|//evil\\.example/x|\\{\\$v\\}|data:,x|rel/x\\.m3u8|http://127\\.0\\.0\\.1:42337@evil\\.example/)";
    let sep = "(\n|\r\n|\u{2028}|\u{2029})";
    (
        tag,
        name,
        gap,
        value,
        proptest::option::of("(\n|\r|\r\n|\u{2028})"),
        sep,
    )
        .prop_map(|(tag, name, gap, value, quote, sep)| {
            let value = match quote {
                Some(brk) => format!("\"x{brk}#EXTINF:4,{brk}{value}{brk}#\""),
                None => value,
            };
            format!("#{tag}{name}{gap}={value}{sep}")
        })
}

proptest::proptest! {
    /// Whatever hostile lines a master or a media playlist carries,
    /// nothing the player reads from the rewrite is fetched past the
    /// proxy.
    #[test]
    fn nothing_the_player_reads_escapes_the_proxy(
        lines in proptest::collection::vec(any_line(), 1..4),
        media in proptest::bool::ANY,
    ) {
        let hostile: String = lines.concat();
        let body = if media {
            format!("#EXTM3U\n#EXT-X-TARGETDURATION:4\n{hostile}#EXTINF:4.0,\nseg0.ts\n#EXT-X-ENDLIST\n")
        } else {
            format!("#EXTM3U\n{hostile}#EXT-X-STREAM-INF:BANDWIDTH=1,AUDIO=\"aud\"\nv.m3u8\n")
        };
        let url = Url::parse("https://cdn.example/a/master.m3u8").expect("url");
        let out = rewrite_served(body.as_bytes(), &url, &origin(), SessionId::new(), &AppSecret::random(), "");
        proptest::prop_assume!(out.is_ok());
        let out = out.expect("rewrite");
        proptest::prop_assert!(escapes(&out).is_empty(), "{:?} in {}", escapes(&out), out);
    }
}
