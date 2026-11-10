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

/// The standard program a command names, when it is only a standard
/// program's name: `mpv`, `vlc` or `iina`, in any case, with or
/// without `.exe` and surrounding spaces. None for anything else: a
/// path, a command with arguments, another program, a blank.
#[must_use]
pub fn standard_name(_command: &str) -> Option<&'static str> {
    None
}

/// Every place `name` may be installed on `os`, in the order they are
/// tried. `var` reads an environment variable.
#[must_use]
pub fn candidates(_name: &str, _os: Os, _var: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    Vec::new()
}

/// The first of `name`'s [`candidates`] that `exists`, then, on
/// Windows, the path its App Paths registration gives (`app_path`,
/// asked with the program's file name, e.g. `vlc.exe`).
#[must_use]
pub fn locate(
    _name: &str,
    _os: Os,
    _var: &dyn Fn(&str) -> Option<String>,
    _exists: &dyn Fn(&str) -> bool,
    _app_path: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    None
}

/// The default value of a `reg query … /ve` answer: the text after its
/// `REG_SZ` or `REG_EXPAND_SZ` type, without surrounding quotes. The
/// value's name column is localized ("(Default)", "(Padrão)", …), so
/// only the type marks the line.
#[must_use]
pub fn parse_reg_default(_output: &str) -> Option<String> {
    None
}

/// Where `name` is installed on this computer, if anywhere.
#[must_use]
pub fn locate_installed(_name: &str) -> Option<String> {
    None
}

#[cfg(test)]
#[path = "player_locate_test.rs"]
mod tests;

#[cfg(test)]
#[path = "player_locate_prop_test.rs"]
mod prop_tests;
