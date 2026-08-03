use super::*;

/// The download arm exactly as the 4.15 release spells it, indentation
/// included. Fixtures build scripts around this constant so none of
/// them can quietly drift from the shape the recognizer requires —
/// [`the_bundled_script_is_recognized`] pins both against the real
/// file.
const BLESSED_ARM: &str = concat!(
    "    download)\n",
    "        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n",
    "        dep_ch \"aria2c\"\n",
    "        ;;\n",
);

/// A minimal script from `version` carrying the release's arm.
fn script_at(version: &str) -> String {
    format!("#!/bin/sh\nversion_number=\"{version}\"\ncase \"$player_function\" in\n{BLESSED_ARM}esac\n")
}

/// The download arm as the 5.0 release spells it: one line, no
/// aria2c. Same discipline as [`BLESSED_ARM`] — fixtures build from
/// this constant so the recognizer and the fixtures cannot drift
/// apart.
const BLESSED_ARM_V5: &str =
    "    download) dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found' ;;\n";

/// A minimal 5.0-shaped script carrying the release's one-line arm.
fn script_v5_at(version: &str) -> String {
    format!("#!/bin/sh\nversion_number=\"{version}\"\ncase \"$player_function\" in\n{BLESSED_ARM_V5}esac\n")
}

/// 5.0 collapsed the download arm to a single line and dropped the
/// aria2c requirement. The recognizer must accept that spelling —
/// the bundled script now carries it — while an edited or reworded
/// variant of it still refuses.
#[test]
fn the_v5_one_line_arm_is_recognized() {
    assert!(supports_ytdlp_download(&script_v5_at("5.0.0")));

    let reworded = BLESSED_ARM_V5.replace("Neither yt-dlp nor ffmpeg found", "no downloader");
    assert!(
        !supports_ytdlp_download(&format!(
            "#!/bin/sh\nversion_number=\"5.0.0\"\ncase \"$player_function\" in\n{reworded}esac\n"
        )),
        "a reworded arm is a customization we cannot vouch for"
    );

    let ffmpeg_only = "#!/bin/sh\nversion_number=\"5.0.0\"\ncase \"$player_function\" in\n    download) dep_ch \"ffmpeg\" ;;\nesac\n";
    assert!(!supports_ytdlp_download(ffmpeg_only));
}

/// Both blessed spellings stay recognized: a user's cache can hold a
/// 4.15 script long after the bundle moves to 5.0.
#[test]
fn the_415_arm_stays_recognized_alongside_v5() {
    assert!(supports_ytdlp_download(&script_at("4.15.0")));
    assert!(supports_ytdlp_download(&script_v5_at("5.0.0")));
}

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
/// script is from the 4.15-or-later lineage, and it still carries that
/// lineage's download arm. Either alone can be true of a script that
/// does not accept yt-dlp.
#[test]
fn both_the_version_and_the_arm_are_required() {
    assert!(supports_ytdlp_download(&script_at("4.15.0")));

    // Right lineage, arm edited out — a customization we cannot vouch
    // for, so it is treated as unknown.
    let no_arm = "#!/bin/sh\nversion_number=\"4.15.0\"\ndownload) dep_ch \"ffmpeg\" ;;\n";
    assert!(!supports_ytdlp_download(no_arm));

    // The arm exists but the script predates the release that made it
    // the download branch's dependency check.
    assert!(!supports_ytdlp_download(&script_at("4.14.0")));
}

/// A later release stays recognized: the failover is not going to be
/// removed upstream, and refusing 4.16 would re-block every user who
/// let the auto-updater run.
#[test]
fn a_later_release_is_still_recognized() {
    for v in ["4.15.1", "4.16.0", "5.0.0", "4.15"] {
        assert!(
            supports_ytdlp_download(&script_at(v)),
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
    let arm = BLESSED_ARM;
    for (label, script) in [
        ("no version line", format!("#!/bin/sh\n{arm}")),
        (
            "unparseable version",
            format!("#!/bin/sh\nversion_number=\"nightly\"\n{arm}"),
        ),
        ("empty", String::new()),
        (
            "version mentioned only in prose",
            format!("#!/bin/sh\n# version_number=\"4.15.0\" is what we target\n{arm}"),
        ),
    ] {
        assert!(
            !supports_ytdlp_download(&script),
            "{label} must fall back to ffmpeg-only"
        );
    }
}

/// The version must be the script's own declaration at the start of a
/// line, not a substring of some other assignment. The arm is present
/// so the version is the only thing under test.
#[test]
fn the_version_is_read_from_its_own_assignment() {
    let shadowed = format!("#!/bin/sh\nprev_version_number=\"4.15.0\"\n{BLESSED_ARM}");
    assert!(
        !supports_ytdlp_download(&shadowed),
        "a different variable is not the script's version"
    );
}

/// A commented-out copy of the arm is documentation, not a dependency
/// check. Accepting one would grant yt-dlp to a script whose real arm
/// still requires ffmpeg — the over-grant direction, where preflight
/// passes on a yt-dlp-only host and the script then exits at its own
/// check.
#[test]
fn a_commented_out_arm_is_not_recognized() {
    let commented: String = BLESSED_ARM.lines().map(|l| format!("# {l}\n")).collect();
    let script = format!(
        "#!/bin/sh\nversion_number=\"4.15.0\"\n{commented}case \"$player_function\" in\n    download) dep_ch \"ffmpeg\" ;;\nesac\n"
    );
    assert!(
        !supports_ytdlp_download(&script),
        "a commented arm is not the dependency check"
    );
}

/// Reindentation is cosmetic and stays recognized; rewording is not.
/// The release's own tail — the redirect and the `die` message — is
/// part of the shape, because "some line that starts with the failover
/// call" is exactly what a documentary excerpt also looks like.
#[test]
fn reindentation_is_allowed_but_rewording_is_not() {
    let reindented: String = BLESSED_ARM
        .lines()
        .map(|l| format!("\t\t{}\n", l.trim()))
        .collect();
    assert!(supports_ytdlp_download(&format!(
        "#!/bin/sh\nversion_number=\"4.15.0\"\n{reindented}"
    )));

    let reworded = BLESSED_ARM.replace("Neither yt-dlp nor ffmpeg found", "no downloader");
    assert!(
        !supports_ytdlp_download(&format!("#!/bin/sh\nversion_number=\"4.15.0\"\n{reworded}")),
        "a reworded arm is a customization we cannot vouch for"
    );
}

/// An excerpt of the arm — a usage block quoting the first line or
/// two, a half-finished edit — is not the arm. This is the shape that
/// actually shows up in the wild, and the one a single-line marker
/// search cannot tell from the real thing.
#[test]
fn a_partial_arm_is_not_recognized() {
    let lines: Vec<&str> = BLESSED_ARM.lines().collect();
    for take in 1..lines.len() {
        let excerpt = lines[..take].join("\n");
        let script = format!("#!/bin/sh\nversion_number=\"4.15.0\"\n{excerpt}\n");
        assert!(
            !supports_ytdlp_download(&script),
            "{take} of {} arm lines is an excerpt, not the arm",
            lines.len()
        );
    }
}

proptest::proptest! {
    /// Recognition tracks the DECLARED version across the range, not
    /// the handful of examples a table test can afford. Every script
    /// here carries the arm verbatim, so the version is the only
    /// variable and the answer must be exactly "at or past 4.15".
    #[test]
    fn recognition_follows_the_declared_version(major in 0u32..12, minor in 0u32..60) {
        let script = script_at(&format!("{major}.{minor}.0"));
        proptest::prop_assert_eq!(
            supports_ytdlp_download(&script),
            (major, minor) >= FAILOVER_RELEASE
        );
    }

    /// Position is not part of the shape: the arm is the arm wherever
    /// it sits, so filler above and below must not change the answer.
    #[test]
    fn the_arm_is_recognized_at_any_position(
        before in proptest::collection::vec("[a-z ]{0,24}", 0..6),
        after in proptest::collection::vec("[a-z ]{0,24}", 0..6),
    ) {
        let script = format!(
            "#!/bin/sh\nversion_number=\"4.15.0\"\n{}\n{BLESSED_ARM}{}\n",
            before.join("\n"),
            after.join("\n")
        );
        proptest::prop_assert!(supports_ytdlp_download(&script));
    }

    /// Dropping ANY line of the arm makes it something other than the
    /// release's dependency check — a documentary excerpt, a
    /// half-edited customization — and the answer must fall back to
    /// ffmpeg-only. This is the direction that matters: a wrongly
    /// granted yt-dlp passes the preflight and then dies in the spawn.
    /// The filler is lowercase words and spaces, so it can never
    /// supply a dropped line back.
    #[test]
    fn a_gapped_arm_never_grants(
        drop_at in 0usize..4,
        filler in proptest::collection::vec("[a-z ]{0,24}", 0..6),
    ) {
        let kept: Vec<&str> = BLESSED_ARM
            .lines()
            .enumerate()
            .filter(|(i, _)| *i != drop_at)
            .map(|(_, l)| l)
            .collect();
        let script = format!(
            "#!/bin/sh\nversion_number=\"4.15.0\"\n{}\n{}\n",
            kept.join("\n"),
            filler.join("\n")
        );
        proptest::prop_assert!(!supports_ytdlp_download(&script));
    }
}
