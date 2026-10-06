//! The binary's contract with the gate: lizard's XML shape on stdout,
//! and a loud failure — never a smaller number — for a file it cannot
//! parse.

use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rust-ccn-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("nested")).expect("mkdir");
    for (path, body) in files {
        std::fs::write(dir.join(path), body).expect("write");
    }
    dir
}

fn run(dir: &PathBuf) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rust-ccn"))
        .arg(dir)
        .output()
        .expect("runs")
}

#[test]
fn every_file_under_the_root_is_reported_in_lizards_shape() {
    let dir = scratch(
        "xml",
        &[
            ("a.rs", "fn one(x: bool) -> u32 { if x { 1 } else { 2 } }\n"),
            ("nested/b.rs", "fn two() {}\n"),
            ("notes.txt", "fn not_rust() {}\n"),
        ],
    );
    let out = run(&dir);
    assert!(out.status.success());
    let xml = String::from_utf8(out.stdout).expect("utf-8");
    let a = dir.join("a.rs");
    let b = dir.join("nested/b.rs");
    assert!(xml.contains(&format!(
        "<item name=\"one(...) at {}:1\">\n\t\t\t<value>1</value>\n\t\t\t<value>1</value>\n\t\t\t<value>2</value>",
        a.display()
    )));
    assert!(xml.contains(&format!("<item name=\"two(...) at {}:1\">", b.display())));
    assert!(!xml.contains("not_rust"));
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(stderr.contains("2 files, 2 functions measured"), "{stderr}");
}

#[test]
fn a_file_that_does_not_parse_fails_the_run_and_names_the_file() {
    let dir = scratch(
        "broken",
        &[("good.rs", "fn fine() {}\n"), ("bad.rs", "fn broken( {\n")],
    );
    let out = run(&dir);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty(), "no partial report");
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(stderr.contains("bad.rs: does not parse"), "{stderr}");
}

#[test]
fn item_position_macros_are_declared_in_the_summary() {
    let dir = scratch(
        "macro",
        &[(
            "m.rs",
            "proptest! { fn holds(x in 0..3) { if x > 1 {} } }\n",
        )],
    );
    let out = run(&dir);
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(
        stderr.contains("1 item-position macro invocations measured as whole blocks"),
        "{stderr}"
    );
}
