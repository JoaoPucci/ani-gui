//! Where an external player's standard program is installed.
//!
//! A blank External player command, or one that is only a standard
//! program's name (`mpv`, `vlc`, `iina`), means "the player of this
//! type that is installed here". A bare name handed to the OS finds a
//! program only on the backend's PATH, and the usual installs are not
//! there: VLC's Windows installer puts it in Program Files without
//! touching PATH, mpv on Windows is often a folder somewhere, and a
//! macOS app started from Finder sees almost no PATH at all. So the
//! program is looked for where each OS puts it, and the result is a
//! full path the spawn cannot miss.
//!
//! Nothing found means the bare name is used as before, so a lookup
//! can never do worse than handing the name to the OS.
//!
//! The search is pure ([`candidates`], [`locate`]): the OS, the
//! environment and the file checks come in as arguments, so Windows
//! and macOS searches are tested on any CI host. [`locate_installed`]
//! wires in the real ones.

/// The OS whose conventions a search follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// Linux: PATH, then the flatpak and snap launchers.
    Linux,
    /// Windows: PATH, the install folders, then App Paths.
    Windows,
    /// macOS: PATH, the app bundles, then Homebrew.
    MacOs,
}

/// The standard programs, by the name a bare command uses.
const STANDARD: [&str; 3] = ["mpv", "vlc", "iina"];

/// The standard program a command names, when it is only a standard
/// program's name: `mpv`, `vlc` or `iina`, in any case, with or
/// without `.exe` and surrounding spaces. None for anything else: a
/// path, a command with arguments, another program, a blank.
#[must_use]
pub fn standard_name(command: &str) -> Option<&'static str> {
    let lower = command.trim().to_ascii_lowercase();
    let name = lower.strip_suffix(".exe").unwrap_or(&lower);
    STANDARD.into_iter().find(|s| *s == name)
}

/// Every place `name` may be installed on `os`, in the order they are
/// tried, each listed once. `var` reads an environment variable.
#[must_use]
pub fn candidates(name: &str, os: Os, var: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |c: String| {
        if !out.contains(&c) {
            out.push(c);
        }
    };
    let (sep, slash, file) = match os {
        Os::Windows => (';', '\\', format!("{name}.exe")),
        Os::Linux | Os::MacOs => (':', '/', name.to_string()),
    };
    for dir in var("PATH").unwrap_or_default().split(sep) {
        let dir = dir.trim().trim_matches('"');
        if !dir.is_empty() {
            push(join(dir, slash, &file));
        }
    }
    for c in installs(name, os, var) {
        push(c);
    }
    out
}

/// `dir` and `file` joined with `slash`, without doubling it.
fn join(dir: &str, slash: char, file: &str) -> String {
    format!("{}{slash}{file}", dir.trim_end_matches(slash))
}

/// The places `os`'s installers put `name`, outside PATH. A place
/// under a variable that is unset is skipped.
fn installs(name: &str, os: Os, var: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let under = |v: &str, rest: &str| var(v).map(|base| join(&base, sep_of(os), rest));
    let fixed = |paths: &[&str]| paths.iter().map(|p| Some((*p).to_string())).collect();
    let found: Vec<Option<String>> = match (os, name) {
        (Os::Windows, "vlc") => vec![
            under("ProgramFiles", "VideoLAN\\VLC\\vlc.exe"),
            under("ProgramFiles(x86)", "VideoLAN\\VLC\\vlc.exe"),
        ],
        (Os::Windows, "mpv") => vec![
            under("ProgramFiles", "mpv\\mpv.exe"),
            under("ProgramFiles(x86)", "mpv\\mpv.exe"),
            under("LOCALAPPDATA", "Programs\\mpv\\mpv.exe"),
            under("USERPROFILE", "scoop\\shims\\mpv.exe"),
            under("LOCALAPPDATA", "Microsoft\\WinGet\\Links\\mpv.exe"),
        ],
        (Os::Linux, "vlc") => flatpak_and_snap("org.videolan.VLC", "vlc", var),
        (Os::Linux, "mpv") => flatpak_and_snap("io.mpv.Mpv", "mpv", var),
        (Os::MacOs, "iina") => bundles("IINA.app/Contents/MacOS/iina-cli", var),
        (Os::MacOs, "vlc") => bundles("VLC.app/Contents/MacOS/VLC", var),
        (Os::MacOs, "mpv") => {
            let mut v = bundles("mpv.app/Contents/MacOS/mpv", var);
            v.extend::<Vec<Option<String>>>(fixed(&[
                "/opt/homebrew/bin/mpv",
                "/usr/local/bin/mpv",
            ]));
            v
        }
        _ => Vec::new(),
    };
    found.into_iter().flatten().collect()
}

fn sep_of(os: Os) -> char {
    match os {
        Os::Windows => '\\',
        Os::Linux | Os::MacOs => '/',
    }
}

/// A Linux program's flatpak launchers (system, then per-user) and its
/// snap launcher.
fn flatpak_and_snap(
    app_id: &str,
    snap: &str,
    var: &dyn Fn(&str) -> Option<String>,
) -> Vec<Option<String>> {
    vec![
        Some(format!("/var/lib/flatpak/exports/bin/{app_id}")),
        var("HOME").map(|h| {
            format!(
                "{}/.local/share/flatpak/exports/bin/{app_id}",
                h.trim_end_matches('/')
            )
        }),
        Some(format!("/snap/bin/{snap}")),
    ]
}

/// A macOS app's program inside its bundle, in /Applications and in
/// the user's own Applications folder.
fn bundles(inner: &str, var: &dyn Fn(&str) -> Option<String>) -> Vec<Option<String>> {
    vec![
        Some(format!("/Applications/{inner}")),
        var("HOME").map(|h| format!("{}/Applications/{inner}", h.trim_end_matches('/'))),
    ]
}

/// The first of `name`'s [`candidates`] that `exists`, then, on
/// Windows, the path its App Paths registration gives (`app_path`,
/// asked with the program's file name, e.g. `vlc.exe`), with its
/// `%VARIABLE%`s expanded.
#[must_use]
pub fn locate(
    name: &str,
    os: Os,
    var: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&str) -> bool,
    app_path: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if let Some(found) = candidates(name, os, var).into_iter().find(|c| exists(c)) {
        return Some(found);
    }
    if os != Os::Windows {
        return None;
    }
    app_path(&format!("{name}.exe"))
        .map(|p| expand(&p, var))
        .filter(|p| exists(p))
}

/// `%NAME%` replaced by the variable's value; an unknown one is kept.
fn expand(text: &str, var: &dyn Fn(&str) -> Option<String>) -> String {
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

/// Where `name` is installed on this computer, if anywhere.
#[must_use]
pub fn locate_installed(name: &str) -> Option<String> {
    let var = |k: &str| std::env::var(k).ok();
    let exists = |p: &str| crate::scraper::fetch::is_executable(std::path::Path::new(p));
    locate(name, current_os(), &var, &exists, &registered_app_path)
}

/// The program Syncplay is told to wrap for a bare standard name, on
/// `os`. Not looked up on Linux, where a sandboxed Syncplay (flatpak,
/// snap) cannot reach a host path, and where its own PATH lookup is
/// what master relied on.
#[must_use]
pub fn syncplay_locate_on(
    os: Os,
    name: &str,
    locate: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    match os {
        Os::Linux => None,
        Os::Windows | Os::MacOs => locate(name),
    }
}

/// [`syncplay_locate_on`] for this computer.
#[must_use]
pub fn locate_for_syncplay(name: &str) -> Option<String> {
    syncplay_locate_on(current_os(), name, &locate_installed)
}

/// The OS this build runs on.
#[must_use]
pub fn current_os() -> Os {
    if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Linux
    }
}

/// The App Paths registration of `file` (e.g. `vlc.exe`): per-user,
/// then machine-wide in the 64-bit and the 32-bit view. Asked through
/// `reg.exe`, without a console window.
#[cfg(windows)]
fn registered_app_path(file: &str) -> Option<String> {
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
fn registered_app_path(_file: &str) -> Option<String> {
    None
}

#[cfg(test)]
#[path = "player_locate_test.rs"]
mod tests;

#[cfg(test)]
#[path = "player_locate_prop_test.rs"]
mod prop_tests;
