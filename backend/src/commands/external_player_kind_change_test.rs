//! What the external player's command becomes when its type changes.

use super::*;
use crate::commands::external_player::ExternalPlayerKind::{Custom, Iina, Mpv, Vlc};

#[test]
fn becomes_the_new_types_bare_command_while_it_is_the_previous_types() {
    assert_eq!(command_after_kind_change("mpv", Mpv, Vlc), "vlc");
    assert_eq!(command_after_kind_change("vlc", Vlc, Iina), "iina");
    assert_eq!(command_after_kind_change("iina", Iina, Mpv), "mpv");
}

#[test]
fn follows_the_type_while_it_is_the_previous_types_name_as_typed() {
    assert_eq!(command_after_kind_change("mpv.exe", Mpv, Vlc), "vlc");
    assert_eq!(command_after_kind_change(" MPV ", Mpv, Iina), "iina");
    assert_eq!(command_after_kind_change("VLC.EXE", Vlc, Mpv), "mpv");
}

#[test]
fn is_never_touched_once_the_user_changed_it() {
    assert_eq!(
        command_after_kind_change("/usr/bin/mpv", Mpv, Vlc),
        "/usr/bin/mpv"
    );
    assert_eq!(
        command_after_kind_change("flatpak run io.mpv.Mpv", Mpv, Iina),
        "flatpak run io.mpv.Mpv"
    );
    assert_eq!(command_after_kind_change("", Mpv, Vlc), "");
    // Another type's bare command is the user's choice for this one.
    assert_eq!(command_after_kind_change("vlc", Mpv, Iina), "vlc");
}

#[test]
fn is_kept_going_to_custom() {
    assert_eq!(command_after_kind_change("mpv", Mpv, Custom), "mpv");
    assert_eq!(
        command_after_kind_change("/usr/bin/vlc", Vlc, Custom),
        "/usr/bin/vlc"
    );
}

#[test]
fn leaving_custom_clears_a_standard_programs_bare_name() {
    assert_eq!(command_after_kind_change("mpv", Custom, Vlc), "");
    assert_eq!(command_after_kind_change("vlc", Custom, Vlc), "");
    assert_eq!(command_after_kind_change("iina", Custom, Mpv), "");
    assert_eq!(command_after_kind_change(" VLC ", Custom, Mpv), "");
    assert_eq!(command_after_kind_change("mpv.exe", Custom, Iina), "");
}

#[test]
fn leaving_custom_keeps_a_command_the_user_wrote() {
    for written in [
        "/usr/bin/mpv",
        "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe",
        "mpv --fs",
        "flatpak run io.mpv.Mpv",
        "celluloid",
        "",
    ] {
        assert_eq!(command_after_kind_change(written, Custom, Vlc), written);
    }
}
