//! Properties of the command a change of player type leaves.

use super::*;
use crate::commands::external_player::ExternalPlayerKind::{self, Custom, Iina, Mpv, Vlc};
use crate::commands::player_locate::standard_name;
use proptest::prelude::*;

fn kind() -> impl Strategy<Value = ExternalPlayerKind> {
    prop_oneof![Just(Mpv), Just(Vlc), Just(Iina), Just(Custom)]
}

fn standard() -> impl Strategy<Value = ExternalPlayerKind> {
    prop_oneof![Just(Mpv), Just(Vlc), Just(Iina)]
}

/// Commands a user may have: the bare defaults, paths, wrappers, blanks.
fn command() -> impl Strategy<Value = String> {
    prop_oneof![
        prop_oneof![
            Just("mpv"),
            Just("vlc"),
            Just("iina"),
            Just(""),
            Just(" mpv "),
            Just("MPV"),
            Just("vlc.exe"),
            Just("/usr/bin/mpv"),
            Just("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe"),
            Just("flatpak run io.mpv.Mpv"),
            Just("/Applications/IINA.app/Contents/MacOS/iina-cli"),
        ]
        .prop_map(str::to_string),
        "\\PC{0,12}",
    ]
}

/// A standard program's bare name as a user may type it.
fn bare_name() -> impl Strategy<Value = String> {
    (
        prop_oneof![
            Just("mpv"),
            Just("vlc"),
            Just("iina"),
            Just("MPV"),
            Just("Vlc"),
            Just("IINA")
        ],
        prop_oneof![Just(""), Just(".exe"), Just(".EXE")],
        "[ \t]{0,2}",
    )
        .prop_map(|(name, ext, pad)| format!("{pad}{name}{ext}{pad}"))
}

proptest! {
    #[test]
    fn every_bare_types_own_command_becomes_every_other_bare_types(
        from in standard(),
        to in standard(),
    ) {
        let own = from.standard_program().expect("a standard type");
        prop_assert_eq!(
            command_after_kind_change(own, from, to),
            to.standard_program().expect("a standard type")
        );
    }

    #[test]
    fn the_command_is_as_it_was_the_new_types_bare_command_or_blank(
        cmd in command(),
        from in kind(),
        to in kind(),
    ) {
        let next = command_after_kind_change(&cmd, from, to);
        prop_assert!(
            next == cmd || Some(next.as_str()) == to.standard_program() || next.is_empty(),
            "{cmd:?} from {from:?} to {to:?} gave {next:?}"
        );
    }

    #[test]
    fn only_the_previous_types_bare_name_or_a_bare_name_leaving_custom_changes(
        cmd in command(),
        from in kind(),
        to in kind(),
    ) {
        let next = command_after_kind_change(&cmd, from, to);
        if next != cmd {
            prop_assert_ne!(to, Custom);
            if from == Custom {
                prop_assert_eq!(next, "");
                prop_assert!(standard_name(&cmd).is_some());
            } else {
                prop_assert_eq!(standard_name(&cmd), from.standard_program());
                prop_assert_eq!(Some(next.as_str()), to.standard_program());
            }
        }
    }

    #[test]
    fn leaving_custom_blanks_exactly_a_bare_name(cmd in command(), bare in bare_name(), to in standard()) {
        prop_assert_eq!(command_after_kind_change(&bare, Custom, to), "");
        let next = command_after_kind_change(&cmd, Custom, to);
        let want = if standard_name(&cmd).is_some() { String::new() } else { cmd.clone() };
        prop_assert_eq!(next, want);
    }

    #[test]
    fn a_bare_command_comes_back_when_the_type_goes_back(
        start in standard(),
        path in proptest::collection::vec(kind(), 0..6),
    ) {
        let mut cmd = start.standard_program().expect("a standard type").to_string();
        let mut at = start;
        for next in path.iter().copied().chain(std::iter::once(start)) {
            cmd = command_after_kind_change(&cmd, at, next);
            at = next;
        }
        // Through Custom the command stays what it was, so only a path
        // that never leaves the bare types is a round trip.
        if !path.contains(&Custom) {
            prop_assert_eq!(Some(cmd.as_str()), start.standard_program());
        }
    }
}
