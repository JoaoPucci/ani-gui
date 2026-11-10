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

#[path = "player_locate_places.rs"]
mod places;
#[path = "player_locate_registry.rs"]
mod registry;
use places::installs;
pub use registry::parse_reg_default;
use registry::{expand, registered_app_path};

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
/// what Watch together always relied on.
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

#[cfg(test)]
#[path = "player_locate_test.rs"]
mod tests;

#[cfg(test)]
#[path = "player_locate_prop_test.rs"]
mod prop_tests;
