//! Tests for `player_locate`: the search for an external player's
//! installed program, on every OS, from any host.

use super::*;
use std::collections::HashMap;

fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    move |k: &str| map.get(k).cloned()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn a_standard_name_is_the_name_alone_in_any_case_with_or_without_exe() {
    assert_eq!(standard_name("mpv"), Some("mpv"));
    assert_eq!(standard_name(" VLC.exe "), Some("vlc"));
    assert_eq!(standard_name("IINA"), Some("iina"));
    assert_eq!(standard_name("Mpv.EXE"), Some("mpv"));
    for other in [
        "",
        "  ",
        "/usr/bin/mpv",
        "C:\\mpv\\mpv.exe",
        "mpv --fs",
        "celluloid",
        "mpvx",
        "vlc.app",
    ] {
        assert_eq!(standard_name(other), None, "{other:?}");
    }
}

#[test]
fn linux_looks_on_path_then_at_the_flatpak_and_snap_launchers() {
    let var = vars(&[("PATH", "/usr/local/bin:/usr/bin"), ("HOME", "/home/u")]);
    assert_eq!(
        candidates("vlc", Os::Linux, &var),
        strings(&[
            "/usr/local/bin/vlc",
            "/usr/bin/vlc",
            "/var/lib/flatpak/exports/bin/org.videolan.VLC",
            "/home/u/.local/share/flatpak/exports/bin/org.videolan.VLC",
            "/snap/bin/vlc",
        ])
    );
    assert_eq!(
        candidates("mpv", Os::Linux, &var),
        strings(&[
            "/usr/local/bin/mpv",
            "/usr/bin/mpv",
            "/var/lib/flatpak/exports/bin/io.mpv.Mpv",
            "/home/u/.local/share/flatpak/exports/bin/io.mpv.Mpv",
            "/snap/bin/mpv",
        ])
    );
}

#[test]
fn iina_exists_only_on_macos() {
    let var = vars(&[("PATH", "/usr/bin"), ("HOME", "/home/u")]);
    assert_eq!(
        candidates("iina", Os::Linux, &var),
        strings(&["/usr/bin/iina"])
    );
    let var = vars(&[("PATH", "C:\\bin"), ("ProgramFiles", "C:\\Program Files")]);
    assert_eq!(
        candidates("iina", Os::Windows, &var),
        strings(&["C:\\bin\\iina.exe"])
    );
}

#[test]
fn windows_looks_on_path_then_in_the_install_folders() {
    let var = vars(&[
        ("PATH", "C:\\Windows\\system32;\"C:\\Tools\\bin\";;C:\\x\\"),
        ("ProgramFiles", "C:\\Program Files"),
        ("ProgramFiles(x86)", "C:\\Program Files (x86)"),
        ("LOCALAPPDATA", "C:\\Users\\u\\AppData\\Local"),
        ("USERPROFILE", "C:\\Users\\u"),
    ]);
    assert_eq!(
        candidates("vlc", Os::Windows, &var),
        strings(&[
            "C:\\Windows\\system32\\vlc.exe",
            "C:\\Tools\\bin\\vlc.exe",
            "C:\\x\\vlc.exe",
            "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe",
            "C:\\Program Files (x86)\\VideoLAN\\VLC\\vlc.exe",
        ])
    );
    assert_eq!(
        candidates("mpv", Os::Windows, &var),
        strings(&[
            "C:\\Windows\\system32\\mpv.exe",
            "C:\\Tools\\bin\\mpv.exe",
            "C:\\x\\mpv.exe",
            "C:\\Program Files\\mpv\\mpv.exe",
            "C:\\Program Files (x86)\\mpv\\mpv.exe",
            "C:\\Users\\u\\AppData\\Local\\Programs\\mpv\\mpv.exe",
            "C:\\Users\\u\\scoop\\shims\\mpv.exe",
            "C:\\Users\\u\\AppData\\Local\\Microsoft\\WinGet\\Links\\mpv.exe",
        ])
    );
}

#[test]
fn a_folder_whose_variable_is_unset_is_skipped() {
    let var = vars(&[("ProgramFiles", "C:\\Program Files")]);
    assert_eq!(
        candidates("vlc", Os::Windows, &var),
        strings(&["C:\\Program Files\\VideoLAN\\VLC\\vlc.exe"])
    );
    assert!(candidates("vlc", Os::Linux, &vars(&[])).contains(&"/snap/bin/vlc".to_string()));
}

#[test]
fn macos_looks_on_path_then_in_the_app_bundles_then_homebrew() {
    let var = vars(&[("PATH", "/usr/bin:/usr/local/bin"), ("HOME", "/Users/u")]);
    assert_eq!(
        candidates("iina", Os::MacOs, &var),
        strings(&[
            "/usr/bin/iina",
            "/usr/local/bin/iina",
            "/Applications/IINA.app/Contents/MacOS/iina-cli",
            "/Users/u/Applications/IINA.app/Contents/MacOS/iina-cli",
        ])
    );
    assert_eq!(
        candidates("vlc", Os::MacOs, &var),
        strings(&[
            "/usr/bin/vlc",
            "/usr/local/bin/vlc",
            "/Applications/VLC.app/Contents/MacOS/VLC",
            "/Users/u/Applications/VLC.app/Contents/MacOS/VLC",
        ])
    );
    // Homebrew's /usr/local/bin is already on this PATH: listed once.
    assert_eq!(
        candidates("mpv", Os::MacOs, &var),
        strings(&[
            "/usr/bin/mpv",
            "/usr/local/bin/mpv",
            "/Applications/mpv.app/Contents/MacOS/mpv",
            "/Users/u/Applications/mpv.app/Contents/MacOS/mpv",
            "/opt/homebrew/bin/mpv",
        ])
    );
}

#[test]
fn locate_takes_the_first_candidate_that_exists() {
    let var = vars(&[("PATH", "/a:/b"), ("HOME", "/home/u")]);
    let exists = |p: &str| p == "/b/mpv" || p == "/snap/bin/mpv";
    let never = |_: &str| -> Option<String> { panic!("App Paths is Windows only") };
    assert_eq!(
        locate("mpv", Os::Linux, &var, &exists, &never),
        Some("/b/mpv".to_string())
    );
    assert_eq!(locate("vlc", Os::Linux, &var, &exists, &never), None);
}

#[test]
fn windows_falls_back_to_the_app_paths_registration() {
    let var = vars(&[("ProgramFiles", "C:\\Program Files"), ("SystemDrive", "D:")]);
    let registered =
        |file: &str| (file == "vlc.exe").then(|| "%SystemDrive%\\Apps\\VLC\\vlc.exe".to_string());
    let exists = |p: &str| p == "D:\\Apps\\VLC\\vlc.exe";
    assert_eq!(
        locate("vlc", Os::Windows, &var, &exists, &registered),
        Some("D:\\Apps\\VLC\\vlc.exe".to_string())
    );
    // A registration pointing at a file that is gone finds nothing.
    let gone = |_: &str| false;
    assert_eq!(locate("vlc", Os::Windows, &var, &gone, &registered), None);
}

#[test]
fn an_install_folder_wins_over_the_registration() {
    let var = vars(&[("ProgramFiles", "C:\\Program Files")]);
    let registered = |_: &str| -> Option<String> { panic!("not needed when a folder has it") };
    let exists = |p: &str| p == "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe";
    assert_eq!(
        locate("vlc", Os::Windows, &var, &exists, &registered),
        Some("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe".to_string())
    );
}

#[test]
fn a_reg_query_answer_gives_its_default_value_whatever_the_language() {
    let english = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\vlc.exe\r\n    (Default)    REG_SZ    C:\\Program Files\\VideoLAN\\VLC\\vlc.exe\r\n\r\n";
    assert_eq!(
        parse_reg_default(english).as_deref(),
        Some("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe")
    );
    let portuguese = "\r\nHKEY_CURRENT_USER\\...\\mpv.exe\r\n    (Padrão)    REG_EXPAND_SZ    \"%LOCALAPPDATA%\\mpv\\mpv.exe\"\r\n";
    assert_eq!(
        parse_reg_default(portuguese).as_deref(),
        Some("%LOCALAPPDATA%\\mpv\\mpv.exe")
    );
    assert_eq!(parse_reg_default(""), None);
    assert_eq!(
        parse_reg_default(
            "ERROR: The system was unable to find the specified registry key or value.\r\n"
        ),
        None
    );
    assert_eq!(parse_reg_default("    (Default)    REG_SZ    \r\n"), None);
}
