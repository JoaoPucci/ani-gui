use super::*;

/// The bundled script must be recognized, or the relaxed preflight
/// silently degrades to ffmpeg-only for every fresh install. This is
/// the reality pin: it reads the actual file rather than a fixture.
#[test]
fn the_bundled_script_is_recognized() {
    let repo_script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root")
        .join("ani-cli");
    let contents = std::fs::read_to_string(repo_script).expect("read repo ani-cli");
    assert!(
        supports_ytdlp_download(&contents),
        "the bundled script must be recognized as yt-dlp-capable"
    );
}

/// Recognition is two independent facts, and BOTH are required: the
/// script is from the 4.15-or-later lineage, and its download arm
/// still carries that lineage's failover call. Either alone can be
/// true of a script that does not accept yt-dlp.
#[test]
fn both_the_version_and_the_failover_line_are_required() {
    let with_both = "#!/bin/sh\nversion_number=\"4.15.0\"\ndep_ch_failover \"yt-dlp,ffmpeg\"\n";
    assert!(supports_ytdlp_download(with_both));

    // Right lineage, arm edited out — a customization we cannot vouch
    // for, so it is treated as unknown.
    let no_line = "#!/bin/sh\nversion_number=\"4.15.0\"\ndep_ch \"ffmpeg\"\n";
    assert!(!supports_ytdlp_download(no_line));

    // The call exists but the script predates the release that made
    // it the download arm's dependency check.
    let old_version = "#!/bin/sh\nversion_number=\"4.14.0\"\ndep_ch_failover \"yt-dlp,ffmpeg\"\n";
    assert!(!supports_ytdlp_download(old_version));
}

/// A later release stays recognized: the failover is not going to be
/// removed upstream, and refusing 4.16 would re-block every user who
/// let the auto-updater run.
#[test]
fn a_later_release_is_still_recognized() {
    for v in ["4.15.1", "4.16.0", "5.0.0", "4.15"] {
        let script =
            format!("#!/bin/sh\nversion_number=\"{v}\"\ndep_ch_failover \"yt-dlp,ffmpeg\"\n");
        assert!(
            supports_ytdlp_download(&script),
            "{v} is at or past the failover release"
        );
    }
}

/// Anything we cannot place is conservative. This is the whole point
/// of the design: an unrecognized script costs a yt-dlp-only user the
/// modal, which is recoverable, rather than passing the preflight and
/// dying inside a spawn, which is not.
#[test]
fn an_unplaceable_script_is_conservative() {
    for (label, script) in [
        ("no version line", "#!/bin/sh\ndep_ch_failover \"yt-dlp,ffmpeg\"\n"),
        (
            "unparseable version",
            "#!/bin/sh\nversion_number=\"nightly\"\ndep_ch_failover \"yt-dlp,ffmpeg\"\n",
        ),
        ("empty", ""),
        (
            "version mentioned only in prose",
            "#!/bin/sh\n# version_number=\"4.15.0\" is what we target\ndep_ch_failover \"yt-dlp,ffmpeg\"\n",
        ),
    ] {
        assert!(
            !supports_ytdlp_download(script),
            "{label} must fall back to ffmpeg-only"
        );
    }
}

/// The version must be the script's own declaration at the start of a
/// line, not a substring of some other assignment.
#[test]
fn the_version_is_read_from_its_own_assignment() {
    let shadowed = "#!/bin/sh\nprev_version_number=\"4.15.0\"\ndep_ch_failover \"yt-dlp,ffmpeg\"\n";
    assert!(
        !supports_ytdlp_download(shadowed),
        "a different variable is not the script's version"
    );
}
