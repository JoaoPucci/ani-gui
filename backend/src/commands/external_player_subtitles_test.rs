//! Sidecar tracks reach each player in the flag it understands.

use super::*;

fn args(kind: ExternalPlayerKind, template: Option<&str>, tracks: &[&str]) -> LaunchArgs {
    LaunchArgs {
        stream_url: "https://cdn.example/x/master.m3u8".into(),
        referer: Some("https://embed.example/".into()),
        title: Some("The Show · ep 1".into()),
        player_command: "player".into(),
        player_kind: kind,
        custom_args_template: template.map(str::to_string),
        subtitle_urls: tracks.iter().map(|t| (*t).to_string()).collect(),
    }
}

const EN: &str = "https://cdn.example/x/subs/en.vtt";
const ES: &str = "https://cdn.example/x/subs/es.vtt";

#[test]
fn mpv_and_iina_take_every_track_as_a_sub_file() {
    let mpv = build_argv(&args(ExternalPlayerKind::Mpv, None, &[EN, ES]));
    assert!(mpv.contains(&format!("--sub-file={EN}")), "{mpv:?}");
    assert!(mpv.contains(&format!("--sub-file={ES}")), "{mpv:?}");
    assert_eq!(
        mpv.last().map(String::as_str),
        Some("https://cdn.example/x/master.m3u8")
    );
    let iina = build_argv(&args(ExternalPlayerKind::Iina, None, &[EN, ES]));
    assert!(iina.contains(&format!("--mpv-sub-file={EN}")), "{iina:?}");
    assert!(iina.contains(&format!("--mpv-sub-file={ES}")), "{iina:?}");
}

#[test]
fn vlc_takes_the_first_track() {
    let vlc = build_argv(&args(ExternalPlayerKind::Vlc, None, &[EN, ES]));
    assert!(vlc.contains(&format!("--sub-file={EN}")), "{vlc:?}");
    assert!(
        !vlc.iter().any(|a| a.contains(ES)),
        "VLC takes one sub-file: {vlc:?}"
    );
}

#[test]
fn no_tracks_add_no_flags() {
    let mpv = build_argv(&args(ExternalPlayerKind::Mpv, None, &[]));
    assert!(!mpv.iter().any(|a| a.contains("sub-file")), "{mpv:?}");
}

#[test]
fn the_custom_template_substitutes_the_first_track_and_drops_the_token_without_one() {
    let with = build_argv(&args(
        ExternalPlayerKind::Custom,
        Some("--sub={subtitle} --ref={referer} {url}"),
        &[EN, ES],
    ));
    assert_eq!(
        with,
        vec![
            format!("--sub={EN}"),
            "--ref=https://embed.example/".to_string(),
            "https://cdn.example/x/master.m3u8".to_string(),
        ]
    );
    let without = build_argv(&args(
        ExternalPlayerKind::Custom,
        Some("--sub={subtitle} {url}"),
        &[],
    ));
    assert_eq!(
        without,
        vec!["https://cdn.example/x/master.m3u8".to_string()]
    );
}

/// What an argv takes on a Windows command line at most, the program
/// included: every character escaped, each argument quoted and spaced.
fn worst_case_line(program: &str, argv: &[String]) -> usize {
    std::iter::once(program)
        .chain(argv.iter().map(String::as_str))
        .map(|a| 2 * a.len() + 3)
        .sum()
}

/// A listing at the cap, each track at the longest URL a CDN signs,
/// would not fit on Windows' command line, and the player would not
/// start. The tracks go on in order until the next would not fit; the
/// stream URL, last, is always there.
#[test]
fn every_track_flag_fits_on_windows_command_line_with_the_stream_last() {
    let long: Vec<String> = (0..64)
        .map(|i| format!("https://cdn.example/{i}/{}.vtt", "a".repeat(1000)))
        .collect();
    let refs: Vec<&str> = long.iter().map(String::as_str).collect();
    for kind in [ExternalPlayerKind::Mpv, ExternalPlayerKind::Iina] {
        let launch = args(kind, None, &refs);
        let argv = build_argv(&launch);
        assert!(
            worst_case_line(&launch.player_command, &argv) <= 32_766,
            "{kind:?}"
        );
        assert_eq!(
            argv.last().map(String::as_str),
            Some("https://cdn.example/x/master.m3u8")
        );
        let subs: Vec<&String> = argv.iter().filter(|a| a.contains("sub-file=")).collect();
        assert!(
            !subs.is_empty() && subs.len() < long.len(),
            "{kind:?}: {}",
            subs.len()
        );
        assert!(
            subs[0].ends_with(&long[0]),
            "the first listed goes on first"
        );
    }
}
