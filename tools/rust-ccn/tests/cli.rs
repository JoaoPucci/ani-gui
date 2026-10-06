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

#[test]
fn a_root_that_does_not_exist_fails_the_run() {
    let out = Command::new(env!("CARGO_BIN_EXE_rust-ccn"))
        .arg("/nonexistent/rust-ccn-root")
        .output()
        .expect("runs");
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(stderr.contains("/nonexistent/rust-ccn-root"), "{stderr}");
}

#[test]
fn a_root_with_no_rust_files_fails_the_run() {
    // A mistyped or emptied root would otherwise score as a language
    // with nothing in it, and the gate would read green on half a
    // repository.
    let dir = scratch("empty", &[("notes.txt", "nothing here\n")]);
    let out = run(&dir);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(stderr.contains("no .rs files"), "{stderr}");
}

#[test]
fn decisions_outside_functions_are_declared_in_the_summary() {
    let dir = scratch(
        "outside",
        &[(
            "o.rs",
            "struct S;\ntrait T {}\nimpl T for S {}\nfn f() {}\n",
        )],
    );
    let out = run(&dir);
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(
        stderr.contains("1 decisions outside any function not counted"),
        "{stderr}"
    );
}

#[test]
fn a_path_of_any_text_is_escaped_into_the_report() {
    // The path sits inside an XML attribute; quotes, ampersands and
    // angle brackets in it must not end or corrupt the attribute.
    let dir = scratch("path", &[]);
    let odd = dir.join("a at b & \"c\" <d>");
    std::fs::create_dir_all(&odd).expect("mkdir");
    std::fs::write(odd.join("e.rs"), "fn r#match() {}\n").expect("write");
    let out = run(&odd);
    assert!(out.status.success());
    let xml = String::from_utf8(out.stdout).expect("utf-8");
    let escaped = odd
        .join("e.rs")
        .display()
        .to_string()
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    assert!(
        xml.contains(&format!("<item name=\"r#match(...) at {escaped}:1\">")),
        "{xml}"
    );
}

#[test]
fn files_that_hold_no_rust_are_listed_as_not_measured() {
    let dir = scratch(
        "data",
        &[("a.rs", "fn a() {}\n"), ("schema.sql", "select 1;\n")],
    );
    let out = run(&dir);
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(
        stderr.contains("not measured — 1 files that hold no Rust"),
        "{stderr}"
    );
    assert!(stderr.contains("schema.sql"), "{stderr}");
}

#[test]
fn a_file_of_a_kind_nobody_decided_on_fails_the_run_and_names_it() {
    // A file that is neither measured nor declared would pass through
    // unmeasured and unreported.
    let dir = scratch("unknown", &[("a.rs", "fn a() {}\n"), ("gen.py", "x = 1\n")]);
    let out = run(&dir);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(
        stderr.contains("gen.py: neither measured nor declared"),
        "{stderr}"
    );
}

#[test]
fn closures_are_counted_and_the_ones_in_macro_input_declared() {
    let dir = scratch(
        "closures",
        &[(
            "c.rs",
            "fn f(xs: &[u32]) -> usize { xs.iter().filter(|x| **x > 1).count() }\n",
        )],
    );
    let out = run(&dir);
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).expect("utf-8");
    assert!(
        stderr.contains("1 files, 1 functions measured; 1 closures measured as units of their own"),
        "{stderr}"
    );
    assert!(
        stderr.contains("closures written inside macro input count for the enclosing unit"),
        "{stderr}"
    );
}
