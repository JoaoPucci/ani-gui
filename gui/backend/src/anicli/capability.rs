//! Does the ACTIVE ani-cli script accept yt-dlp alone for downloads?
//!
//! Deliberately not a shell parser. An earlier version walked the
//! script's case statements to find the download arm and read its
//! dependency check, which meant approximating POSIX grammar —
//! quoting, heredocs, arithmetic, command substitution, line
//! continuations — to answer one boolean about a script this repo
//! bundles and pins. Every corner of that grammar was another way to
//! get the answer wrong, in both directions.
//!
//! Recognition replaces it. We know which release made
//! `dep_ch_failover "yt-dlp,ffmpeg"` the download arm's dependency
//! check, and we know how that release spells the whole arm, so the
//! question becomes: is this script from that lineage, and does it
//! still carry that arm verbatim? Anything else is unplaceable and
//! treated as ffmpeg-only.
//!
//! The arm, not the call, is the unit. A single blessed line is also
//! what a `usage()` heredoc quoting the arm looks like, and what a
//! half-finished customization leaves behind — and granting on either
//! passes the preflight against a script whose real arm still calls
//! `dep_ch "ffmpeg"`.
//!
//! The asymmetry is what makes conservatism the right default: a
//! wrongly-withheld grant costs a yt-dlp-only user the missing-ffmpeg
//! modal, which they can act on, while a wrongly-given one passes the
//! preflight and then dies inside the spawn with a generic error.
//! The cost is real, though — a customized or reformatted script
//! stops being recognized until we sync — so the pin below is
//! load-bearing and should move when the bundled script does.

/// The release that introduced the yt-dlp-or-ffmpeg failover in the
/// download arm. Scripts at or past this carry it.
const FAILOVER_RELEASE: (u32, u32) = (4, 15);

/// Each blessed release's download arm, line by line, spelled as that
/// release spells it. The whole arm is the unit of recognition rather
/// than the failover call alone: "a line that starts with
/// `dep_ch_failover`" is also what a usage block quoting the arm looks
/// like, and what a half-finished customization leaves behind.
/// Granting on either would pass the preflight on a yt-dlp-only host
/// against a script whose real arm still calls `dep_ch "ffmpeg"`.
///
/// Two lineages are blessed: 4.15's four-line arm (with its aria2c
/// check) and 5.0's single line, which dropped aria2c along with the
/// provider rewrite. Both stay listed because a user's cache can hold
/// the older script long after the bundle moves on.
///
/// Lines are compared trimmed and in order, so reindentation is
/// cosmetic and rewording is not.
const DOWNLOAD_ARMS: &[&[&str]] = &[
    &[
        "download)",
        r#"dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'"#,
        r#"dep_ch "aria2c""#,
        ";;",
    ],
    &[
        r#"download) dep_ch_failover "yt-dlp,ffmpeg" >/dev/null || die 'Neither yt-dlp nor ffmpeg found' ;;"#,
    ],
];

/// Whether the script accepts yt-dlp alone for `-d` downloads.
pub(crate) fn supports_ytdlp_download(script_contents: &str) -> bool {
    let Some((major, minor)) = declared_version(script_contents) else {
        return false;
    };
    if (major, minor) < FAILOVER_RELEASE {
        return false;
    }
    carries_download_arm(script_contents)
}

/// Whether any blessed release's download arm appears verbatim, as
/// consecutive lines.
///
/// What this deliberately does NOT do is decide whether those lines
/// are reached at runtime — that is the shell grammar this module
/// exists to avoid. A script that reproduces an arm byte-for-byte
/// inside a heredoc *while* editing its real arm to require ffmpeg
/// would still be granted. Customization does not produce that shape;
/// quoting an excerpt does, and an excerpt no longer matches.
fn carries_download_arm(script_contents: &str) -> bool {
    let lines: Vec<&str> = script_contents.lines().map(str::trim).collect();
    DOWNLOAD_ARMS
        .iter()
        .any(|arm| lines.windows(arm.len()).any(|w| &w == arm))
}

/// The script's own `version_number="X.Y..."` declaration, as
/// (major, minor). Must start its line — a different variable whose
/// name merely ends in `version_number` is not the script's version.
fn declared_version(script_contents: &str) -> Option<(u32, u32)> {
    let raw = script_contents
        .lines()
        .find_map(|l| l.strip_prefix("version_number="))?
        .trim()
        .trim_matches(['"', '\'']);
    let mut parts = raw.split('.');
    let major = parts.next()?.parse().ok()?;
    // A two-component version is legal; treat the missing minor as 0.
    let minor = parts.next().map_or(Some(0), |m| m.parse().ok())?;
    Some((major, minor))
}

#[cfg(test)]
#[path = "capability_test.rs"]
mod tests;
