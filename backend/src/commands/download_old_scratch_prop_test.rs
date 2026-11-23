//! Which names the old-scratch sweep takes for an earlier version's
//! leftovers: every name those versions wrote, and nothing else.

use super::is_old_scratch;
use proptest::prelude::*;

fn old_key(pid: u32, id: u128) -> String {
    format!(".ani-gui-{pid}-{id:032x}")
}

/// What an earlier version, or a tool it ran, wrote after the key.
fn old_tail() -> impl Strategy<Value = String> {
    let file = prop_oneof![
        Just("mp4".to_string()),
        (
            // Any id yt-dlp could put in a name: no separator, no
            // control character, none of what it replaces in one.
            "[^/\\\\\x00-\x1f\x7f\u{80}-\u{9f}\\s,+\\[\\]()]{1,20}",
            "(mp4|m4a|m4v|ts|aac|mp3|webm|mkv)"
        )
            .prop_map(|(id, ext)| format!("f{id}.{ext}")),
    ];
    let tool = prop_oneof![
        Just(String::new()),
        Just(".part".to_string()),
        Just(".ytdl".to_string()),
        (0u32..10_000).prop_map(|n| format!(".part-Frag{n}")),
        (0u32..10_000).prop_map(|n| format!(".part-Frag{n}.part")),
    ];
    prop_oneof![
        Just(".part.vtt".to_string()),
        Just(".part.temp.mp4".to_string()),
        (file, tool).prop_map(|(f, t)| format!(".part.{f}{t}")),
    ]
}

proptest! {
    /// Every name an earlier version could leave is one.
    #[test]
    fn every_name_an_earlier_version_left_is_its_scratch(
        pid in any::<u32>(),
        id in any::<u128>(),
        tail in old_tail(),
    ) {
        let name = format!("{}{tail}", old_key(pid, id));
        prop_assert!(is_old_scratch(&name), "{name}");
    }

    /// This version's names — a 16-digit key, its hold and every part
    /// under it, whatever a tool appends — are never one.
    #[test]
    fn this_versions_names_never_are(
        pid in any::<u32>(),
        id in any::<u64>(),
        n in any::<u32>(),
        suffix in "[^/\\\\\x00]{0,40}",
    ) {
        let key = format!(".ani-gui-{pid}-{id:016x}");
        let hold = format!("{key}.hold");
        prop_assert!(!is_old_scratch(&hold));
        let part = format!("{key}-{n}{suffix}");
        prop_assert!(!is_old_scratch(&part), "{part}");
    }

    /// An old name with its key's hex out of shape is not one: an
    /// uppercase digit, one too few, one too many.
    #[test]
    fn a_key_out_of_shape_is_never_one(
        pid in any::<u32>(),
        id in any::<u128>(),
        tail in old_tail(),
        at in 0usize..32,
        extra in "[0-9a-f]",
    ) {
        let hex = format!("{id:032x}");
        let mut upper = hex.clone().into_bytes();
        upper[at] = b'A' + (upper[at] % 6);
        let upper = String::from_utf8(upper).expect("ascii");
        let short = &hex[..31];
        let long = format!("{hex}{extra}");
        for id in [upper.as_str(), short, long.as_str()] {
            let name = format!(".ani-gui-{pid}-{id}{tail}");
            prop_assert!(!is_old_scratch(&name), "{name}");
        }
    }

    /// An old name with anything a user's tool might add after it — a
    /// backup's `~`, a copy's ` (1)`, a further extension — is not one.
    #[test]
    fn anything_after_an_old_name_makes_it_not_one(
        pid in any::<u32>(),
        id in any::<u128>(),
        tail in old_tail(),
        after in prop_oneof![
            Just("~".to_string()),
            Just(" (1)".to_string()),
            Just(".".to_string()),
            Just(".bak".to_string()),
            Just(".txt".to_string()),
        ],
    ) {
        let name = format!("{}{tail}{after}", old_key(pid, id));
        prop_assert!(!is_old_scratch(&name), "{name}");
    }

    /// A copy of a split stream's leftover, as a file manager or a
    /// browser names one — its text before the extension — is not one.
    #[test]
    fn a_copy_of_a_split_streams_leftover_is_never_one(
        pid in any::<u32>(),
        id in "[a-z0-9-]{1,12}",
        ext in "(mp4|m4a|ts)",
        copy in prop_oneof![
            Just(" (1)"),
            Just(" (2)"),
            Just(" - Copy"),
            Just(" (copy)"),
            Just(" copy"),
        ],
    ) {
        let name = format!("{}.part.f{id}{copy}.{ext}", old_key(pid, 7));
        prop_assert!(!is_old_scratch(&name), "{name}");
    }

    /// A name that does not start with the old prefix is never one,
    /// whatever follows.
    #[test]
    fn a_name_without_the_prefix_is_never_one(
        lead in "[^.][^/\\\\\x00]{0,8}",
        pid in any::<u32>(),
        id in any::<u128>(),
        tail in old_tail(),
    ) {
        let name = format!("{lead}{}{tail}", old_key(pid, id));
        prop_assert!(!is_old_scratch(&name), "{name}");
    }
}

#[test]
fn names_near_the_old_shape_are_not_its_scratch() {
    let hex = "0123456789abcdef0123456789abcdef";
    for name in [
        // The key alone, and the key with a finished episode's name.
        format!(".ani-gui-42-{hex}"),
        format!(".ani-gui-42-{hex}.mp4"),
        format!(".ani-gui-42-{hex}.part"),
        // No pid, a pid too long for a u32, a pid that is not digits.
        format!(".ani-gui--{hex}.part.mp4"),
        format!(".ani-gui-12345678901-{hex}.part.mp4"),
        format!(".ani-gui-x42-{hex}.part.mp4"),
        // A container yt-dlp does not write a stream in, and a format
        // id with no extension or no id.
        format!(".ani-gui-42-{hex}.part.fhls-720.exe"),
        format!(".ani-gui-42-{hex}.part.fhls-720"),
        format!(".ani-gui-42-{hex}.part.f.mp4"),
        // A fragment with no number, a resume state on a vtt.
        format!(".ani-gui-42-{hex}.part.mp4.part-Frag"),
        format!(".ani-gui-42-{hex}.part.vtt.ytdl"),
        format!(".ani-gui-42-{hex}.part.mp4.part.part"),
        // A copy of a stream's leftover, renamed before its extension.
        format!(".ani-gui-42-{hex}.part.fhls-720 (1).mp4"),
        format!(".ani-gui-42-{hex}.part.fhls-720 - Copy.mp4"),
        format!(".ani-gui-42-{hex}.part.fhls-720,1.mp4"),
        format!(".ani-gui-42-{hex}.part.fhls-720+1.mp4"),
        format!(".ani-gui-42-{hex}.part.fhls-720[1].mp4"),
        // The folders this version and the last kept.
        ".ani-gui-parts".to_string(),
        ".ani-gui-locks".to_string(),
        // The user's own files.
        "Frieren Episode 1.mp4".to_string(),
        "Frieren Episode 1.part.mp4".to_string(),
        "ani-gui-42-0123456789abcdef0123456789abcdef.part.mp4".to_string(),
    ] {
        assert!(!is_old_scratch(&name), "{name}");
    }
}

#[test]
fn the_names_an_earlier_version_wrote_are_its_scratch() {
    let hex = "0123456789abcdef0123456789abcdef";
    for tail in [
        ".part.mp4",
        ".part.mp4.part",
        ".part.mp4.ytdl",
        ".part.mp4.part-Frag12",
        ".part.mp4.part-Frag16.part",
        ".part.temp.mp4",
        ".part.vtt",
        ".part.fhls-1080p.mp4",
        ".part.fhls-audio_aac-English.m4a.part",
        ".part.fhls-2.5.mp4.part-Frag3.part",
        // yt-dlp builds an HLS id from the rendition's group and name,
        // replacing only whitespace and `,/+[]()` with `_` (and on
        // Windows the characters Windows forbids in a name, with `#`),
        // so an id can carry any letter.
        ".part.fhls-audio-日本語.m4a.part",
        ".part.fhls-audio-Español.m4a.ytdl",
        ".part.fhls-audio#1.m4a",
    ] {
        let name = format!(".ani-gui-4242-{hex}{tail}");
        assert!(is_old_scratch(&name), "{name}");
    }
}
