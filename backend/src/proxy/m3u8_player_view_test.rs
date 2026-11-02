//! The oracle flags what hls.js would fetch past the proxy. The
//! rewrite's own output never gives it one, so these feed it text
//! directly.

use super::escapes;

#[test]
fn the_oracle_flags_what_the_player_would_fetch_past_the_proxy() {
    let media = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n";
    assert_eq!(
        escapes(&format!("{media}#EXTINF:4https://evil.example/x.ts\n")),
        ["https://evil.example/x.ts"]
    );
    assert_eq!(
        escapes(&format!("{media}#EXT-X-MAP:URI=\"{{$v}}\"\n")),
        ["{$v}"]
    );
    assert_eq!(
        escapes(&format!("{media}#EXT-X-MAP:URI=\"http://[\"\n")),
        ["http://["]
    );
    assert!(escapes(&format!(
        "{media}#EXT-X-MAP:URI=\"data:,x\"\n#EXTINF:4,\ndata:,y\n"
    ))
    .is_empty());
    assert_eq!(
        escapes("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\ndata:,y\n"),
        ["data:,y"]
    );
}
