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

/// The App Paths registration of `file` (e.g. `vlc.exe`): per-user,
/// then machine-wide in the 64-bit and the 32-bit view. Asked through
/// `reg.exe`, without a console window.
#[cfg(windows)]
pub(super) fn registered_app_path(file: &str) -> Option<String> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let key = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{file}");
    let reg = std::env::var("SystemRoot")
        .map(|root| format!("{root}\\System32\\reg.exe"))
        .unwrap_or_else(|_| "reg.exe".to_string());
    let queries: [(&str, Option<&str>); 3] = [
        ("HKCU", None),
        ("HKLM", Some("/reg:64")),
        ("HKLM", Some("/reg:32")),
    ];
    queries.into_iter().find_map(|(hive, view)| {
        let mut cmd = std::process::Command::new(&reg);
        cmd.args(["query", &format!("{hive}\\{key}"), "/ve"]);
        if let Some(view) = view {
            cmd.arg(view);
        }
        let out = cmd
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| parse_reg_default(&String::from_utf8_lossy(&out.stdout)))
            .flatten()
    })
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
