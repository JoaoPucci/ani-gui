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
