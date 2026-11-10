//! Properties of `player_program`: what an external-player launch runs.

use super::*;
use proptest::prelude::*;

fn kind() -> impl Strategy<Value = ExternalPlayerKind> {
    prop_oneof![
        Just(ExternalPlayerKind::Mpv),
        Just(ExternalPlayerKind::Vlc),
        Just(ExternalPlayerKind::Iina),
        Just(ExternalPlayerKind::Custom),
    ]
}

/// The program each type runs when the command is blank.
fn standard(kind: ExternalPlayerKind) -> Option<&'static str> {
    match kind {
        ExternalPlayerKind::Mpv => Some("mpv"),
        ExternalPlayerKind::Vlc => Some("vlc"),
        ExternalPlayerKind::Iina => Some("iina"),
        ExternalPlayerKind::Custom => None,
    }
}

proptest! {
    #[test]
    fn a_command_with_text_runs_exactly_as_written(
        command in "[ ]{0,2}[A-Za-z0-9/._:-][A-Za-z0-9/._: -]{0,20}[ ]{0,2}",
        kind in kind(),
    ) {
        prop_assert_eq!(player_program(&command, kind), Some(command.as_str()));
    }

    #[test]
    fn a_blank_command_runs_the_types_standard_program(
        blank in "[ \t]{0,4}",
        kind in kind(),
    ) {
        prop_assert_eq!(player_program(&blank, kind), standard(kind));
    }

    #[test]
    fn the_program_is_the_command_or_the_standard_one(
        command in ".{0,24}",
        kind in kind(),
    ) {
        let program = player_program(&command, kind);
        prop_assert!(program == Some(command.as_str()) || program == standard(kind));
    }
}
