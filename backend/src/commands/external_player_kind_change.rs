//! What the external player's command becomes when the user changes the
//! player's type in Settings (mpv, VLC, IINA, Custom).
//!
//! While the command is still the previous type's bare command ("mpv",
//! "vlc", "iina", in any case, with or without ".exe" and surrounding
//! spaces), it becomes the new type's: a user who never wrote one gets
//! the player they picked. Between mpv, VLC and IINA, a command the
//! user wrote (a full path, a wrapper, a blank, another type's name) is
//! theirs, and is never touched. Custom has no command of its own, so a
//! change to it keeps the command.
//!
//! A blank command runs the type's standard program
//! ([`super::player_program`]). So leaving Custom clears a command that
//! is only a standard program's name to blank, and the new type runs
//! its own: "mpv" left over from Custom would otherwise run mpv under
//! the VLC type. A command with a path or flags is kept. No path is
//! guessed here: where a bare name is installed is
//! [`crate::commands::player_locate`]'s to find, at launch.

use super::ExternalPlayerKind;

/// The command after the type changes from `from` to `to`.
#[must_use]
pub fn command_after_kind_change(
    command: &str,
    from: ExternalPlayerKind,
    to: ExternalPlayerKind,
) -> String {
    let _ = (from, to);
    command.to_string()
}

#[cfg(test)]
#[path = "external_player_kind_change_test.rs"]
mod tests;

#[cfg(test)]
#[path = "external_player_kind_change_prop_test.rs"]
mod prop_tests;
