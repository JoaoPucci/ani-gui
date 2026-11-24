//! Which program a launch runs: the configured command, or a blank
//! one's standard program, found where it is installed
//! ([`crate::commands::player_locate`]); and what Settings is told a
//! blank command runs. Split from [`super`] so each file stays inside
//! the CRAP gate's per-file bar.

use serde::Serialize;

use super::ExternalPlayerKind;

/// The program a launch runs: the command the user configured, or,
/// when it is blank, the type's standard program. A blank Custom
/// command runs nothing: Custom has no standard program.
#[must_use]
pub fn player_program(command: &str, kind: ExternalPlayerKind) -> Option<&str> {
    if command.trim().is_empty() {
        kind.standard_program()
    } else {
        Some(command)
    }
}

/// Settings' answer when the lookup task dies before answering.
#[must_use]
pub fn standard_program_unknown(kind: ExternalPlayerKind) -> StandardProgram {
    standard_program_found_on(crate::commands::player_locate::current_os(), kind, &|_| {
        None
    })
}

/// The program a launch spawns: [`player_program`], with a standard
/// program's bare name replaced by where `locate` finds it installed.
/// A name `locate` cannot find is spawned as it is.
#[must_use]
pub fn launch_program_with(
    command: &str,
    kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let program = player_program(command, kind)?;
    Some(installed_or_as_is(program, locate))
}

/// `program` as `locate` finds it installed, when it is a standard
/// program's bare name. Otherwise, or when nothing is found, as it is.
fn installed_or_as_is(program: &str, locate: &dyn Fn(&str) -> Option<String>) -> String {
    crate::commands::player_locate::standard_name(program)
        .and_then(locate)
        .unwrap_or_else(|| program.to_string())
}

/// The player Syncplay is told to wrap (`--player-path`), from the
/// configured command.
#[must_use]
pub fn syncplay_player_with(
    command: &str,
    _kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> String {
    // Blank: no --player-path, so Syncplay opens the player configured
    // in Syncplay itself, as it always did.
    if command.trim().is_empty() {
        return String::new();
    }
    installed_or_as_is(command, locate)
}

/// What a blank command runs for `kind` on this computer: the standard
/// program's name, and where `locate` finds it installed. Settings
/// shows it under the External player field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StandardProgram {
    /// The program's name (`"vlc"`); none for Custom.
    pub name: Option<&'static str>,
    /// Where it is installed; none when it was not found.
    pub path: Option<String>,
    /// Whether the program exists for this OS at all (IINA is
    /// macOS-only).
    pub supported: bool,
}

/// [`StandardProgram`] for `kind`, found by `locate`.
#[must_use]
pub fn standard_program_found(
    kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> StandardProgram {
    standard_program_found_on(crate::commands::player_locate::current_os(), kind, locate)
}

/// [`standard_program_found`] on `os`.
#[must_use]
pub fn standard_program_found_on(
    os: crate::commands::player_locate::Os,
    kind: ExternalPlayerKind,
    locate: &dyn Fn(&str) -> Option<String>,
) -> StandardProgram {
    let name = kind.standard_program();
    // IINA is a macOS app: there is nothing to find anywhere else.
    let supported =
        !(kind == ExternalPlayerKind::Iina && os != crate::commands::player_locate::Os::MacOs);
    StandardProgram {
        name,
        path: if supported {
            name.and_then(locate)
        } else {
            None
        },
        supported,
    }
}

impl ExternalPlayerKind {
    /// The program this type runs when the user wrote no command: its
    /// own name, looked for where it is installed
    /// ([`crate::commands::player_locate`]). None for Custom.
    #[must_use]
    pub fn standard_program(self) -> Option<&'static str> {
        match self {
            Self::Mpv => Some("mpv"),
            Self::Vlc => Some("vlc"),
            Self::Iina => Some("iina"),
            Self::Custom => None,
        }
    }
}
