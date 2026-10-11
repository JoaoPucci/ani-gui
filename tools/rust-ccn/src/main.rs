//! `rust-ccn [--tsv] <path>...` — measure every `.rs` file under the
//! given paths. Default output is lizard's XML shape, so the CRAP
//! scorer reads it exactly as it reads lizard's; `--tsv` lists one unit
//! per line. A file that does not parse is reported and the run exits
//! non-zero: an unparsed file is a measurement that did not happen, and
//! the gate must not read it as zero. So is a root that does not exist
//! or holds no `.rs` file: a mistyped path would otherwise score the
//! language as empty. Every other file is either a declared data kind,
//! listed as not measured, or fails the run naming it.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rust_ccn::{measure_file, Unit, UnitKind};

fn main() -> ExitCode {
    let mut tsv = false;
    let mut roots = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--tsv" {
            tsv = true;
        } else {
            roots.push(PathBuf::from(arg));
        }
    }
    if roots.is_empty() {
        eprintln!("usage: rust-ccn [--tsv] <path>...");
        return ExitCode::from(2);
    }
    let mut found = Found::default();
    for root in &roots {
        let before = found.rust.len();
        if let Err(err) = std::fs::metadata(root).and_then(|_| collect(root, &mut found)) {
            eprintln!("rust-ccn: {}: {err}", root.display());
            return ExitCode::FAILURE;
        }
        if found.rust.len() == before {
            eprintln!("rust-ccn: {}: no .rs files to measure", root.display());
            return ExitCode::FAILURE;
        }
    }
    if !found.unknown.is_empty() {
        for f in &found.unknown {
            eprintln!(
                "rust-ccn: {}: neither measured nor declared — measure its kind or declare it in tools/rust-ccn",
                f.display()
            );
        }
        return ExitCode::FAILURE;
    }
    let mut files = found.rust;
    files.sort();
    let mut no_rust = found.no_rust;
    no_rust.sort();

    let mut measured = Vec::new();
    let mut outside = 0;
    let mut failed = false;
    for file in &files {
        let result = std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|src| {
                measure_file(&src).map_err(|e| {
                    let at = e.span().start();
                    format!("{}:{}: {e}", at.line, at.column + 1)
                })
            });
        match result {
            Ok(m) => {
                outside += m.outside;
                measured.push((file.clone(), m.units));
            }
            Err(err) => {
                eprintln!("rust-ccn: {}: does not parse: {err}", file.display());
                failed = true;
            }
        }
    }
    if failed {
        return ExitCode::FAILURE;
    }
    let out = if tsv {
        render_tsv(&measured)
    } else {
        render_xml(&measured)
    };
    print!("{out}");
    eprintln!("{}", summary(&measured, outside));
    if !no_rust.is_empty() {
        eprintln!(
            "rust-ccn: not measured — {} files that hold no Rust (data: {}):",
            no_rust.len(),
            NO_RUST.join(", ")
        );
        for f in &no_rust {
            eprintln!("  {}", f.display());
        }
    }
    ExitCode::SUCCESS
}

/// What was measured, and what was not: closures written inside macro
/// input are not parsed and count for the enclosing unit, functions
/// written inside an item-position macro's input are counted as part of
/// that macro's block, not one by one, and decisions outside every unit
/// are not counted at all.
fn summary(measured: &[(PathBuf, Vec<Unit>)], outside: u32) -> String {
    let all = measured.iter().flat_map(|(_, units)| units);
    let functions = all.clone().filter(|u| u.kind == UnitKind::Function).count();
    let closures = all.clone().filter(|u| u.kind == UnitKind::Closure).count();
    let macros = all.filter(|u| u.kind == UnitKind::Macro).count();
    let mut line = format!(
        "rust-ccn: {} files, {functions} functions measured; {closures} closures measured as \
         units of their own (closures written inside macro input count for the enclosing unit)",
        measured.len()
    );
    if outside > 0 {
        line.push_str(&format!(
            "; {outside} decisions outside any function not counted \
             (e.g. the `for` of `impl Trait for Type`)"
        ));
    }
    if macros > 0 {
        line.push_str(&format!(
            "; {macros} item-position macro invocations measured as whole blocks \
             (functions inside macro input are not visible to a parser one by one)"
        ));
    }
    line
}

/// Files that hold no Rust and are listed as not measured: data a crate
/// embeds or ships beside its sources. Any other non-`.rs` kind fails
/// the run, so a new kind gets a decision rather than a silent pass.
const NO_RUST: &[&str] = &["sql", "json", "toml", "txt", "md"];

#[derive(Default)]
struct Found {
    rust: Vec<PathBuf>,
    no_rust: Vec<PathBuf>,
    unknown: Vec<PathBuf>,
}

fn collect(path: &Path, out: &mut Found) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            collect(&entry?.path(), out)?;
        }
        return Ok(());
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let bucket = if ext == "rs" {
        &mut out.rust
    } else if NO_RUST.contains(&ext) {
        &mut out.no_rust
    } else {
        &mut out.unknown
    };
    bucket.push(path.to_path_buf());
    Ok(())
}

fn render_tsv(measured: &[(PathBuf, Vec<Unit>)]) -> String {
    let mut out = String::new();
    for (file, units) in measured {
        for u in units {
            let kind = match u.kind {
                UnitKind::Function => "fn",
                UnitKind::Macro => "macro",
                UnitKind::Closure => "closure",
            };
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                file.display(),
                kind,
                report_name(&u.name),
                u.line,
                u.end_line,
                u.ccn
            ));
        }
    }
    out
}

/// A unit's name as the report carries it: ASCII name characters only,
/// so the scorer can find where the name ends. ASCII identifiers pass
/// through (`r#match`, `proptest!`); a non-ASCII one (`café`) has those
/// characters replaced, which costs only the diagnostic name.
fn report_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_$#()!".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// A path inside the report's XML attribute.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_xml(measured: &[(PathBuf, Vec<Unit>)]) -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" ?>\n<cppncss>\n\t<measure type=\"Function\">\n");
    let mut nr = 0;
    for (file, units) in measured {
        for u in units {
            nr += 1;
            out.push_str(&format!(
                "\t\t<item name=\"{}(...) at {}:{}\">\n\t\t\t<value>{nr}</value>\n\t\t\t<value>{}</value>\n\t\t\t<value>{}</value>\n\t\t</item>\n",
                report_name(&u.name),
                xml_escape(&file.display().to_string()),
                u.line,
                u.end_line + 1 - u.line,
                u.ccn
            ));
        }
    }
    out.push_str("\t</measure>\n</cppncss>\n");
    out
}
