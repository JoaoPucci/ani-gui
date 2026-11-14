//! `media_url`: the proxy URL a session's media is fetched at.

use super::media_url;
use crate::proxy::MediaKind;

proptest::proptest! {
    /// The URL is the proxy origin, the session's own path, and the one
    /// file the proxy serves the kind's media at — whatever the origin
    /// and the session id, so the renderer's copy of the shape
    /// (`frontend/src/lib/play/media-url.ts`) can compose it from them.
    #[test]
    fn a_session_plays_at_its_kinds_file_under_its_own_path(
        base in "http://127\\.0\\.0\\.1:[0-9]{1,5}",
        session in "[0-9a-f]{1,32}",
        mp4 in proptest::bool::ANY,
    ) {
        let (kind, file) = if mp4 {
            (MediaKind::Mp4, "file.mp4")
        } else {
            (MediaKind::Hls, "master.m3u8")
        };
        let url = media_url(&base, &session, kind);
        proptest::prop_assert_eq!(url, format!("{base}/s/{session}/{file}"));
    }
}
