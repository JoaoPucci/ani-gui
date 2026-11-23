//! The Windows App Paths registration, read through `reg.exe`. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

/// The default value of a `reg query … /ve` answer: the text after its
/// `REG_SZ` or `REG_EXPAND_SZ` type, without surrounding quotes. The
/// value's name column is localized ("(Default)", "(Padrão)", …), so
/// only the type marks the line.
#[must_use]
pub fn parse_reg_default(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, value) = line
            .split_once("    REG_EXPAND_SZ    ")
            .or_else(|| line.split_once("    REG_SZ    "))?;
        let value = value.trim().trim_matches('"').trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// The key's default value from a `reg export` file: the file's bytes,
/// UTF-16 as reg.exe writes them, so a path in any language comes back
/// as written. The first key's `@` line only; a REG_SZ value is quoted
/// and escaped, a REG_EXPAND_SZ one written as `hex(2):` bytes.
#[must_use]
pub fn parse_reg_export(bytes: &[u8]) -> Option<String> {
    let body = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let text = String::from_utf16(&units).ok()?;
    // A long value is continued over lines ending in `\`.
    let joined = text.replace("\\\r\n", "").replace("\\\n", "");
    let mut keys = 0;
    for line in joined.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            keys += 1;
            continue;
        }
        if keys != 1 {
            continue;
        }
        let Some(value) = line.strip_prefix("@=") else {
            continue;
        };
        let value = if let Some(quoted) = value.strip_prefix('"') {
            unescape_reg_string(quoted.strip_suffix('"')?)
        } else {
            decode_hex2(value.strip_prefix("hex(2):")?)?
        };
        let value = value.trim().trim_matches('"').trim();
        return (!value.is_empty()).then(|| value.to_string());
    }
    None
}

/// A quoted .reg value's text: `\\` and `\"` stand for `\` and `"`.
fn unescape_reg_string(escaped: &str) -> String {
    let mut out = String::new();
    let mut chars = escaped.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A `hex(2):` value: comma-separated bytes of UTF-16LE text, up to its
/// first NUL.
fn decode_hex2(list: &str) -> Option<String> {
    let bytes: Vec<u8> = list
        .split(',')
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(|b| u8::from_str_radix(b, 16).ok())
        .collect::<Option<_>>()?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}

/// How long one `reg.exe` query may take before it is given up and
/// killed. It answers in milliseconds; a hung one must not hold
/// Settings or a launch.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows query
const REG_LIMIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The App Paths registration of `file` (e.g. `vlc.exe`): per-user,
/// then machine-wide in the 64-bit and the 32-bit view. Exported
/// through `reg.exe` ([`reg_command`], [`reg_output`]), without a
/// console window, each export bounded by [`REG_LIMIT`], and read as
/// the UTF-16 file it writes ([`parse_reg_export`]): its query output
/// is in the console's code page, which cannot carry every path.
#[cfg(windows)]
pub(super) fn registered_app_path(file: &str) -> Option<String> {
    let key = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{file}");
    let reg = std::env::var("SystemRoot")
        .map(|root| format!("{root}\\System32\\reg.exe"))
        .unwrap_or_else(|_| "reg.exe".to_string());
    let queries: [(&str, Option<&str>); 3] = [
        ("HKCU", None),
        ("HKLM", Some("/reg:64")),
        ("HKLM", Some("/reg:32")),
    ];
    let file = export_file();
    let found = queries.into_iter().find_map(|(hive, view)| {
        let mut args = vec![
            "export".to_string(),
            format!("{hive}\\{key}"),
            file.display().to_string(),
            "/y".to_string(),
        ];
        args.extend(view.map(str::to_string));
        let out = reg_output(reg_command(&reg, &args), REG_LIMIT)?;
        if !out.status.success() {
            return None;
        }
        parse_reg_export(&std::fs::read(&file).ok()?)
    });
    let _ = std::fs::remove_file(&file);
    found
}

/// A file of this process's own in the temporary folder, for one
/// lookup's exports: reg.exe writes an export only to a file.
#[cfg(windows)]
fn export_file() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ani-gui-app-paths-{}-{n}.reg", std::process::id()))
}

/// The `reg.exe` command for one query (AGENTS.md §5): a tokio
/// command, killed if dropped, with `TERM=dumb` and `NO_COLOR=1`, no
/// stdin, and on Windows no console window. Its output is collected by
/// `output()`, which captures stdout and stderr; only stdout is read.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows query and the tests
pub(super) fn reg_command(program: &str, args: &[String]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// The output of `cmd`, or none when it can't start or runs past
/// `limit`, in which case it is dropped and so killed. Runs on a thread
/// of its own with its own small runtime, so it can be called from any
/// context, a blocking-pool thread or inside an async task alike.
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows query and the tests
pub(super) fn reg_output(
    mut cmd: tokio::process::Command,
    limit: std::time::Duration,
) -> Option<std::process::Output> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        runtime.block_on(async move { tokio::time::timeout(limit, cmd.output()).await.ok()?.ok() })
    })
    .join()
    .ok()
    .flatten()
}

/// No registry off Windows.
#[cfg(not(windows))]
pub(super) fn registered_app_path(_file: &str) -> Option<String> {
    None
}

/// `%NAME%` replaced by the variable's value; an unknown one is kept.
pub(super) fn expand(text: &str, var: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(len) = after.find('%') else { break };
        out.push_str(&rest[..start]);
        let key = &after[..len];
        match var(key) {
            Some(v) if !key.is_empty() => out.push_str(&v),
            _ => {
                out.push('%');
                out.push_str(key);
                out.push('%');
            }
        }
        rest = &after[len + 1..];
    }
    out.push_str(rest);
    out
}
