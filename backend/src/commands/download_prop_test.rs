//! Property coverage for the download command's pure helpers.

use super::{ffmpeg_referer_args, ytdlp_referer_args};
use proptest::prelude::*;

proptest! {
    /// Whatever the referer is, yt-dlp receives it as the operand
    /// of its own flag, byte for byte; no referer means no
    /// argument at all.
    #[test]
    fn ytdlp_gets_the_referer_verbatim_behind_its_flag(referer in "[^\x00]{0,80}") {
        prop_assert_eq!(
            ytdlp_referer_args(Some(&referer)),
            vec!["--referer".to_string(), referer.clone()]
        );
        prop_assert!(ytdlp_referer_args(None).is_empty());
    }

    /// ffmpeg receives the referer as one raw header line,
    /// CRLF-terminated, behind its headers flag; no referer means
    /// no argument at all.
    #[test]
    fn ffmpeg_gets_the_referer_as_one_header_line(referer in "[^\x00]{0,80}") {
        prop_assert_eq!(
            ffmpeg_referer_args(Some(&referer)),
            vec!["-headers".to_string(), format!("Referer: {referer}\r\n")]
        );
        prop_assert!(ffmpeg_referer_args(None).is_empty());
    }
}
