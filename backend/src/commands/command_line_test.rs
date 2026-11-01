//! The hand-off's subtitle flags fitted to the command line Windows
//! allows: 32,766 UTF-16 units for a program, 8,191 characters for a
//! batch file run through `cmd.exe`.

use super::flags_within_budget;
use proptest::prelude::*;

/// Whether Windows runs `program` through `cmd.exe`: by its extension
/// once the path is resolved, which strips trailing dots and spaces.
fn is_batch(program: &str) -> bool {
    let lower = program
        .trim()
        .trim_end_matches(['.', ' '])
        .to_ascii_lowercase();
    lower.ends_with(".bat") || lower.ends_with(".cmd")
}

/// What `arg` can take on `program`'s Windows command line at most,
/// written out here, not read from the code under test. Rust's std
/// quotes an argument and escapes its quotes and backslashes, at most
/// doubling it; for a batch file it escapes each `%` as `%%cd:~,%`,
/// eight units for one byte. Each argument is quoted and spaced.
fn worst(program: &str, arg: &str) -> usize {
    let per_byte = if is_batch(program) { 8 } else { 2 };
    per_byte * arg.len() + 3
}

/// The line `program` runs with: itself — for a batch file, as
/// `cmd.exe /e:ON /v:OFF /d /c "` and the script's resolved path,
/// quoted and not escaped, which the configured name may not show, so
/// counted at no less than a 260-unit path — then its arguments.
fn total(program: &str, fixed: &[String], flags: &[String]) -> usize {
    let head = if is_batch(program) {
        32 + program.len().max(260) + 3
    } else {
        worst(program, program)
    };
    head + fixed.iter().map(|a| worst(program, a)).sum::<usize>()
        + flags.iter().map(|a| worst(program, a)).sum::<usize>()
}

fn flag(i: usize, len: usize) -> String {
    let stem = format!("--sub-file=https://cdn.example/{i}/");
    let pad = len.saturating_sub(stem.len() + 4);
    format!("{stem}{}.vtt", "a".repeat(pad))
}

/// A listing of 33 real-length tracks fits whole.
#[test]
fn a_real_listing_fits_whole() {
    let fixed = vec![
        "--force-media-title=Steel Ball Run · ep 1".to_string(),
        "--referrer=https://megaplay.buzz/".to_string(),
        format!(
            "https://ncdn.imgnex.top/anime/{}/master.m3u8",
            "f".repeat(65)
        ),
    ];
    let flags: Vec<String> = (0..33).map(|i| flag(i, 140)).collect();
    assert_eq!(flags_within_budget("mpv", &fixed, flags.clone()), flags);
}

/// Sixty-four tracks at the longest URL a CDN signs would take about
/// 130,000 units: the most wanted ones go on until the next would not
/// fit, and the line stays under Windows' bound.
#[test]
fn a_listing_of_the_longest_urls_is_cut_to_the_bound() {
    let fixed = vec!["https://cdn.example/master.m3u8".to_string()];
    let flags: Vec<String> = (0..64).map(|i| flag(i, 1024 + 11)).collect();
    let kept = flags_within_budget("mpv", &fixed, flags.clone());
    assert!(!kept.is_empty() && kept.len() < flags.len());
    assert_eq!(kept[..], flags[..kept.len()], "the most wanted, in order");
    assert!(total("mpv", &fixed, &kept) <= 32_766);
    assert!(total("mpv", &fixed, &flags[..=kept.len()]) > 32_766);
}

/// A batch file runs through `cmd.exe`, whose command is shorter.
#[test]
fn a_batch_file_gets_cmds_bound() {
    let fixed = vec!["https://cdn.example/master.m3u8".to_string()];
    let flags: Vec<String> = (0..64).map(|i| flag(i, 300)).collect();
    for program in [r"C:\tools\mpv.bat", r"C:\tools\MPV.CMD", "mpv.cmd "] {
        let kept = flags_within_budget(program, &fixed, flags.clone());
        assert!(total(program, &fixed, &kept) <= 8_191, "{program}");
        assert!(
            total(program, &fixed, &flags[..=kept.len()]) > 8_191,
            "{program}"
        );
    }
}

/// A signed URL carries `%`-escapes, which a batch file's line writes
/// eight units long: the cut counts them so, and stays under
/// `cmd.exe`'s bound where a count of two units a byte would not.
#[test]
fn a_batch_files_percent_signs_are_counted_at_their_escaped_length() {
    let fixed = vec!["https://cdn.example/master.m3u8".to_string()];
    let flags: Vec<String> = (0..16)
        .map(|i| {
            format!(
                "--sub-file=https://cdn.example/{i}/{}.vtt?s={}",
                "a".repeat(100),
                "%2F".repeat(60)
            )
        })
        .collect();
    let kept = flags_within_budget(r"C:\tools\mpv.bat", &fixed, flags.clone());
    assert!(total(r"C:\tools\mpv.bat", &fixed, &kept) <= 8_191);
    assert!(kept.len() < flags.len());
}

/// A trailing dot or space is stripped when Windows resolves the
/// path, so `run.bat.` still runs through `cmd.exe`, and gets its
/// bound.
#[test]
fn a_batch_file_named_with_a_trailing_dot_gets_cmds_bound() {
    let fixed = vec!["https://cdn.example/master.m3u8".to_string()];
    let flags: Vec<String> = (0..64).map(|i| flag(i, 300)).collect();
    for program in ["run.bat.", "run.cmd. ", r"C:\x\run.BAT.."] {
        let kept = flags_within_budget(program, &fixed, flags.clone());
        assert!(total(program, &fixed, &kept) <= 8_191, "{program}");
    }
}

/// A batch file's own path is quoted on its line, not escaped: a long
/// path costs its length, not eight times it.
#[test]
fn a_batch_files_path_is_counted_at_its_length() {
    let fixed: Vec<String> = Vec::new();
    let flags: Vec<String> = (0..2000).map(|i| format!("-s{i:04}")).collect();
    let short = flags_within_budget("run.bat", &fixed, flags.clone()).len();
    let long =
        flags_within_budget(&format!(r"C:\{}\run.bat", "d".repeat(300)), &fixed, flags).len();
    assert!(short > long, "a longer path leaves less room");
    assert!(
        short - long <= 1,
        "{short} vs {long}: the path is counted unescaped"
    );
}

proptest! {
    /// Whatever the program, the fixed arguments and the flags: what
    /// is kept is a leading run of the flags, the whole fits the
    /// program's bound, and the next flag, if any, would not have.
    #[test]
    fn the_kept_flags_are_the_longest_leading_run_that_fits(
        program in prop_oneof![
            Just("mpv".to_string()),
            Just("/usr/bin/syncplay".to_string()),
            Just(r"C:\x\run.bat".to_string()),
            "[A-Za-z0-9:/\\\\. ]{0,40}",
        ],
        fixed in proptest::collection::vec("[ -~]{0,400}", 0..5),
        lens in proptest::collection::vec(10usize..1100, 0..70),
    ) {
        let flags: Vec<String> = lens.iter().enumerate().map(|(i, &n)| flag(i, n)).collect();
        let kept = flags_within_budget(&program, &fixed, flags.clone());
        prop_assert!(kept.len() <= flags.len());
        prop_assert_eq!(&kept[..], &flags[..kept.len()]);
        let bound = if is_batch(&program) { 8_191 } else { 32_766 };
        let base = total(&program, &fixed, &[]);
        if base <= bound {
            prop_assert!(total(&program, &fixed, &kept) <= bound);
        } else {
            prop_assert!(kept.is_empty());
        }
        if kept.len() < flags.len() {
            prop_assert!(total(&program, &fixed, &flags[..=kept.len()]) > bound);
        }
    }
}
