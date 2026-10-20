//! What a test's stub tool writes for the test to read back: its
//! argv, a pid. The stub runs beside the test — detached, or still
//! running when the test polls for the file — so the test can read the
//! file while the stub is writing it.

/// `word` as one POSIX shell word that reads back as exactly `word`:
/// single-quoted, with each apostrophe in it closed out, escaped and
/// reopened (`'\''`). Inside single quotes nothing else is special —
/// not `$`, a backtick, a double quote, a backslash or a space — so a
/// path embedded in a generated script this way is that path whatever
/// it is named.
///
/// # Panics
/// On a word that is not UTF-8, which the script it goes into could
/// not carry as written.
pub(crate) fn sh_quote(word: impl AsRef<std::ffi::OsStr>) -> String {
    let word = word
        .as_ref()
        .to_str()
        .expect("a word embedded in a stub script is UTF-8");
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// `word` as a TOML string value that reads back as exactly `word`
/// (the toml crate picks the quoting form),
/// for a config file a case writes with a staged path in it. A path
/// with a double quote or a backslash in it would otherwise end the
/// string early or start an escape.
///
/// # Panics
/// On a word that is not UTF-8, which TOML cannot carry.
pub(crate) fn toml_quote(word: impl AsRef<std::ffi::OsStr>) -> String {
    let word = word
        .as_ref()
        .to_str()
        .expect("a word embedded in a config file is UTF-8");
    toml::Value::String(word.to_owned()).to_string()
}

/// `word` as a POSIX extended regular expression that matches exactly
/// `word`: every character the syntax gives a meaning is escaped. For
/// a staged path a stub hands a matcher like `pgrep -f`, where a `.` or
/// a `$` in a directory's name would otherwise match something else
/// or nothing.
///
/// # Panics
/// On a word that is not UTF-8.
pub(crate) fn ere_literal(word: impl AsRef<std::ffi::OsStr>) -> String {
    let word = word
        .as_ref()
        .to_str()
        .expect("a word matched by a stub script is UTF-8");
    let mut out = String::with_capacity(word.len());
    for c in word.chars() {
        // `]` and `}` are ordinary outside a bracket expression or a
        // bound, and escaping them is undefined in POSIX.
        if "\\.[()*+?{|^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A shell line that publishes `producer`'s output at `path` whole:
/// written beside it, then renamed over it, so a reader finds either
/// no file or the complete one — never one still being written.
pub(crate) fn publish(path: &std::path::Path, producer: &str) -> String {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    format!(
        "{producer} > {part} && mv -f {part} {path}",
        part = sh_quote(&part),
        path = sh_quote(path)
    )
}

/// A file written in place is readable half-written: the redirection
/// creates it empty and the shell fills it as it writes. macOS's
/// `/bin/sh` is bash, which line-buffers its stdout, so
/// `printf '%s\n'` reaches the file one line — one argument — at a
/// time, and a test that takes the first non-empty read can read only
/// the first argument. Published whole, the file at `path` is
/// replaced, never written: a name linked to the file that was there
/// before still sees that file, empty, and `path` reads complete.
#[test]
fn a_published_file_replaces_the_one_at_its_path_rather_than_writing_into_it() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("argv");
    let witness = dir.path().join("witness");
    std::fs::write(&path, b"").expect("seed");
    std::fs::hard_link(&path, &witness).expect("link");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(publish(&path, "printf '%s\\n' \"$@\""))
        .args(["sh", "--player-path=mpv", "--", "http://x/master.m3u8"])
        .status()
        .expect("sh");
    assert!(status.success());
    assert_eq!(
        std::fs::read_to_string(&path).expect("published"),
        "--player-path=mpv\n--\nhttp://x/master.m3u8\n"
    );
    assert_eq!(
        std::fs::read_to_string(&witness).expect("witness"),
        "",
        "the file was written in place, where a reader can see it half written"
    );
}

/// Every staged path comes from the system's temporary directory, and
/// nothing keeps that directory's name free of what a shell reads
/// specially: an apostrophe ends a single-quoted word, `$` and a
/// backtick expand inside double quotes, a space splits an unquoted
/// one. A path published under such a name still lands whole, at
/// exactly that name.
#[test]
fn a_file_is_published_under_a_directory_whose_name_a_shell_reads_specially() {
    let root = tempfile::tempdir().expect("tmp");
    let dir = root.path().join("it's a $HOME `x` \"q\" \\ dir");
    std::fs::create_dir(&dir).expect("odd dir");
    let path = dir.join("argv");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(publish(&path, "printf '%s\\n' \"$@\""))
        .args(["sh", "--player-path=mpv", "http://x/master.m3u8"])
        .status()
        .expect("sh");
    assert!(status.success(), "the line runs under that name");
    assert_eq!(
        std::fs::read_to_string(&path).expect("published at the path"),
        "--player-path=mpv\nhttp://x/master.m3u8\n"
    );
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]

    // Whatever the word holds, the shell reads the quoted form back as
    // that word, in one piece.
    #[test]
    fn a_quoted_word_reads_back_as_itself(word in "[ -~\n\t]{0,40}") {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf '%s' {}", sh_quote(&word)))
            .output()
            .expect("sh");
        proptest::prop_assert!(out.status.success());
        proptest::prop_assert_eq!(String::from_utf8(out.stdout).expect("utf-8"), word);
    }

    // The escaped form matches the word and only text that contains it.
    #[test]
    fn an_escaped_word_matches_itself_literally(word in "[ -~]{1,30}") {
        let pattern = ere_literal(&word);
        let found = |line: &str| {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("printf '%s\\n' {} | grep -Eq -- {}", sh_quote(line), sh_quote(&pattern)))
                .status()
                .expect("sh")
                .success()
        };
        let around = format!("x{word}y");
        proptest::prop_assert!(found(&around));
        let mangled: String = word.chars().map(|c| if c == 'z' { 'y' } else { 'z' }).collect();
        proptest::prop_assume!(!mangled.contains(&word));
        proptest::prop_assert!(!found(&mangled));
    }

    // A TOML string built from the word parses back as the word.
    #[test]
    fn a_toml_quoted_word_parses_back_as_itself(word in "[\\PC\n\t]{0,40}") {
        let doc: toml::Value = toml::from_str(&format!("v = {}", toml_quote(&word))).expect("toml");
        proptest::prop_assert_eq!(doc.get("v").and_then(toml::Value::as_str), Some(word.as_str()));
    }
}
