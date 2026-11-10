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

proptest! {
    /// Never worse than before the lookup: with nothing found, a
    /// launch spawns exactly what `player_program` named.
    #[test]
    fn with_nothing_installed_a_launch_is_what_it_was(
        command in ".{0,24}",
        kind in kind(),
    ) {
        let nothing = |_: &str| None;
        prop_assert_eq!(
            launch_program_with(&command, kind, &nothing),
            player_program(&command, kind).map(str::to_string)
        );
    }

    /// Only a blank field or a bare standard name is ever looked up,
    /// and what is found is what launches.
    #[test]
    fn only_a_blank_or_bare_name_is_replaced_by_what_is_found(
        command in prop_oneof![
            Just(String::new()),
            Just("mpv".to_string()),
            Just(" VLC ".to_string()),
            Just("iina.exe".to_string()),
            ".{0,24}",
        ],
        kind in kind(),
    ) {
        let found = |n: &str| Some(format!("/found/{n}"));
        let got = launch_program_with(&command, kind, &found);
        let looked_up = command.trim().is_empty() && kind.standard_program().is_some()
            || crate::commands::player_locate::standard_name(&command).is_some();
        if looked_up {
            prop_assert!(got.as_deref().is_some_and(|g| g.starts_with("/found/")));
        } else {
            prop_assert_eq!(got, player_program(&command, kind).map(str::to_string));
        }
    }
}
