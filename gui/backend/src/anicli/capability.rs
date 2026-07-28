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
//! check, and we know how that release spells it, so the question
//! becomes: is this script from that lineage, and does it still carry
//! that call? Anything else is unplaceable and treated as
//! ffmpeg-only.
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

/// The 4.15 download arm's dependency check, spelled as that release
/// spells it. Matched literally, and only where it OPENS a line
/// (after indentation): a commented-out copy is not a dependency
/// check, and accepting one would grant yt-dlp to a script whose real
/// arm still requires ffmpeg. A script that words it differently is a
/// customization we cannot vouch for.
const FAILOVER_CALL: &str = r#"dep_ch_failover "yt-dlp,ffmpeg""#;

/// Whether the script accepts yt-dlp alone for `-d` downloads.
pub(crate) fn supports_ytdlp_download(script_contents: &str) -> bool {
    let Some((major, minor)) = declared_version(script_contents) else {
        return false;
    };
    if (major, minor) < FAILOVER_RELEASE {
        return false;
    }
    script_contents
        .lines()
        .any(|l| l.trim_start().starts_with(FAILOVER_CALL))
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
