//! Properties of the installed-program search.

use super::*;
use proptest::prelude::*;

fn os() -> impl Strategy<Value = Os> {
    prop_oneof![Just(Os::Linux), Just(Os::Windows), Just(Os::MacOs)]
}

fn name() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("mpv"), Just("vlc"), Just("iina")]
}

proptest! {
    #[test]
    fn a_standard_name_survives_any_case_padding_and_exe(
        n in name(),
        upper in proptest::collection::vec(any::<bool>(), 3),
        exe in prop_oneof![Just(""), Just(".exe"), Just(".EXE"), Just(".Exe")],
        pad in "[ \\t]{0,3}",
    ) {
        let cased: String = n
            .chars()
            .zip(upper.iter().cycle())
            .map(|(c, up)| if *up { c.to_ascii_uppercase() } else { c })
            .collect();
        prop_assert_eq!(standard_name(&format!("{pad}{cased}{exe}{pad}")), Some(n));
    }

    #[test]
    fn a_command_with_a_separator_or_a_space_inside_is_never_a_standard_name(
        n in name(),
        before in "[a-z/\\\\ ]{1,6}",
    ) {
        let command = format!("{before}{n}");
        prop_assume!(command.trim() != n);
        prop_assert_eq!(standard_name(&command), None);
    }

    #[test]
    fn path_comes_first_in_its_own_order(
        os in os(),
        n in name(),
        dirs in proptest::collection::vec("[a-z]{1,8}", 1..5),
    ) {
        let (sep, root, slash, file) = match os {
            Os::Windows => (";", "C:\\", "\\", format!("{n}.exe")),
            _ => (":", "/", "/", n.to_string()),
        };
        let full: Vec<String> = dirs.iter().map(|d| format!("{root}{d}")).collect();
        let path = full.join(sep);
        let var = move |k: &str| (k == "PATH").then(|| path.clone());
        let got = candidates(n, os, &var);
        let mut want: Vec<String> = Vec::new();
        for d in &full {
            let c = format!("{d}{slash}{file}");
            if !want.contains(&c) {
                want.push(c);
            }
        }
        prop_assert_eq!(&got[..want.len()], &want[..]);
    }

    #[test]
    fn locate_is_the_first_existing_candidate_or_nothing(
        os in prop_oneof![Just(Os::Linux), Just(Os::MacOs)],
        n in name(),
        dirs in proptest::collection::vec("[a-z]{1,6}", 0..4),
        present in proptest::collection::vec(any::<bool>(), 12),
    ) {
        let path = dirs.iter().map(|d| format!("/{d}")).collect::<Vec<_>>().join(":");
        let var = move |k: &str| match k {
            "PATH" => Some(path.clone()),
            "HOME" => Some("/home/u".to_string()),
            _ => None,
        };
        let all = candidates(n, os, &var);
        let exists_set: Vec<String> = all
            .iter()
            .zip(present.iter().cycle())
            .filter(|(_, p)| **p)
            .map(|(c, _)| c.clone())
            .collect();
        let exists = |p: &str| exists_set.iter().any(|e| e == p);
        let none = |_: &str| -> Registrations { Box::new(std::iter::empty()) };
        prop_assert_eq!(locate(n, os, &var, &exists, &none), exists_set.first().cloned());
    }

    #[test]
    fn syncplay_looks_up_off_linux_only_and_takes_the_answer_as_is(
        os in os(),
        n in name(),
        answer in proptest::option::of("[a-zA-Z0-9/\\\\:. ]{1,30}"),
    ) {
        let asked = std::cell::Cell::new(false);
        let given = answer.clone();
        let locate = |q: &str| {
            asked.set(true);
            assert_eq!(q, n);
            given.clone()
        };
        let got = syncplay_locate_on(os, n, &locate);
        if os == Os::Linux {
            prop_assert!(!asked.get());
            prop_assert_eq!(got, None);
        } else {
            prop_assert_eq!(got, answer);
        }
    }

    #[test]
    fn an_exported_value_comes_back_as_written_in_either_form(
        value in "[A-Za-z]:\\\\[A-Za-z0-9 ()._çãñВидео\\\\-]{0,40}[A-Za-z0-9.]",
        expand in any::<bool>(),
    ) {
        let line = if expand {
            let mut hex = String::from("hex(2):");
            let units: Vec<u16> = value.encode_utf16().chain([0]).collect();
            let bytes: Vec<String> = units
                .iter()
                .flat_map(|u| u.to_le_bytes())
                .map(|b| format!("{b:02x}"))
                .collect();
            for (i, b) in bytes.iter().enumerate() {
                if i > 0 {
                    hex.push(',');
                    if i % 24 == 0 {
                        hex.push_str("\\\r\n  ");
                    }
                }
                hex.push_str(b);
            }
            format!("@={hex}")
        } else {
            format!("@=\"{}\"", value.replace('\\', "\\\\"))
        };
        let text = format!("Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\X]\r\n{line}\r\n\r\n");
        let mut file = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            file.extend_from_slice(&unit.to_le_bytes());
        }
        prop_assert_eq!(parse_reg_export(&file), Some(value));
    }
}
