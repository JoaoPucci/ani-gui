//! Tests for `crate::parser`. Extracted via `#[path]` so the inline
//! `mod tests { ... }` block doesn't count toward the file's CCN — per
//! `project_crap_inline_test_gotcha`.

use super::*;

#[test]
fn strip_ansi_removes_escape_codes() {
    let raw = b"\x1b[1;31mred\x1b[0m text";
    let out = strip_ansi(raw);
    assert_eq!(out, "red text");
}

#[test]
fn parse_search_one_line() {
    let line = "abc123\tOne Piece (1100 episodes)";
    let parsed = parse_search_results(line);
    assert_eq!(parsed.len(), 1);
    let r = &parsed[0];
    assert_eq!(r.id, "abc123");
    assert_eq!(r.title, "One Piece");
    assert_eq!(r.episode_count, 1100);
}

#[test]
fn parse_search_handles_parens_in_title() {
    // The title contains its own parentheses; only the last `(N
    // episodes)` group is the count.
    let line = "xyz\tFullmetal Alchemist (Brotherhood) (64 episodes)";
    let parsed = parse_search_results(line);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].title, "Fullmetal Alchemist (Brotherhood)");
    assert_eq!(parsed[0].episode_count, 64);
}

#[test]
fn parse_search_skips_non_matching_lines() {
    let stdout = "Checking dependencies...\n\
                      abc\tFoo (12 episodes)\n\
                      garbage line without tab\n\
                      def\tBar (1 episode)\n";
    let parsed = parse_search_results(stdout);
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].id, "abc");
    assert_eq!(parsed[1].id, "def");
    // Singular "1 episode" is accepted too.
    assert_eq!(parsed[1].episode_count, 1);
}

#[test]
fn parse_debug_minimal() {
    let stdout = "All links:\n\
                      720 >https://example.com/720.mp4\n\
                      Selected link:\n\
                      https://example.com/720.mp4\n";
    let d = parse_debug_output(stdout).unwrap();
    assert_eq!(d.selected_url, "https://example.com/720.mp4");
    assert_eq!(
        d.all_links,
        vec!["720 >https://example.com/720.mp4".to_string()]
    );
    assert_eq!(d.referer, None);
    assert_eq!(d.subtitle_url, None);
}

#[test]
fn parse_debug_with_m3u8_subs_and_refr() {
    let stdout = "All links:\n\
                      1080cc>https://example.com/1080.m3u8\n\
                      720cc>https://example.com/720.m3u8\n\
                      subtitle >https://example.com/sub.vtt\n\
                      m3u8_refr >https://allmanga.to\n\
                      Selected link:\n\
                      https://example.com/1080.m3u8\n";
    let d = parse_debug_output(stdout).unwrap();
    assert_eq!(d.selected_url, "https://example.com/1080.m3u8");
    assert_eq!(
        d.subtitle_url.as_deref(),
        Some("https://example.com/sub.vtt")
    );
    assert_eq!(d.referer.as_deref(), Some("https://allmanga.to"));
    // subtitle and m3u8_refr lines are stripped from all_links.
    assert!(d.all_links.iter().all(|l| !l.starts_with("subtitle >")));
    assert!(d.all_links.iter().all(|l| !l.starts_with("m3u8_refr >")));
    assert_eq!(d.all_links.len(), 2);
}

#[test]
fn parse_debug_missing_marker_errors() {
    let stdout = "Some output but no Selected marker\n";
    let err = parse_debug_output(stdout).unwrap_err();
    match err {
        AniError::ParseFailed { detail } => {
            assert!(
                detail.contains("Selected link"),
                "detail mentions marker: {detail}"
            );
        }
        other => panic!("expected ParseFailed, got {other:?}"),
    }
}
