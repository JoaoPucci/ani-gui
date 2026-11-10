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

#[test]
fn syncplay_on_linux_gets_the_name_as_typed() {
    // A flatpak or snap Syncplay cannot reach a host path, and its own
    // PATH lookup is what a bare name always relied on there.
    let never = |_: &str| -> Option<String> { panic!("not looked up on Linux") };
    assert_eq!(syncplay_locate_on(Os::Linux, "mpv", &never), None);
}

#[test]
fn syncplay_on_windows_and_macos_gets_the_installed_program() {
    let found = |n: &str| Some(format!("/found/{n}"));
    assert_eq!(
        syncplay_locate_on(Os::Windows, "vlc", &found).as_deref(),
        Some("/found/vlc")
    );
    assert_eq!(
        syncplay_locate_on(Os::MacOs, "iina", &found).as_deref(),
        Some("/found/iina")
    );
}

#[test]
fn a_registered_path_keeps_a_variable_it_cannot_expand_and_a_lone_percent() {
    let var = vars(&[("ProgramFiles", "C:\\Program Files")]);
    for (registered, expanded) in [
        ("%NOPE%\\VLC\\vlc.exe", "%NOPE%\\VLC\\vlc.exe"),
        ("C:\\100%\\vlc.exe", "C:\\100%\\vlc.exe"),
        ("%%\\vlc.exe", "%%\\vlc.exe"),
        (
            "%ProgramFiles%\\VLC\\vlc.exe",
            "C:\\Program Files\\VLC\\vlc.exe",
        ),
    ] {
        let app = |_: &str| Some(registered.to_string());
        let exists = |p: &str| p == expanded;
        assert_eq!(
            locate("vlc", Os::Windows, &var, &exists, &app).as_deref(),
            Some(expanded),
            "{registered}"
        );
    }
}

#[test]
fn the_registry_query_follows_the_subprocess_convention() {
    // AGENTS.md §5: tokio's Command, killed on drop, TERM=dumb, NO_COLOR=1.
    let args = strings(&["query", "HKCU\\X", "/ve"]);
    let cmd = registry::reg_command("reg.exe", &args);
    assert!(cmd.get_kill_on_drop());
    let std = cmd.as_std();
    let envs: HashMap<String, Option<String>> = std
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert_eq!(envs.get("TERM"), Some(&Some("dumb".to_string())));
    assert_eq!(envs.get("NO_COLOR"), Some(&Some("1".to_string())));
    let got: Vec<String> = std
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(got, args);
}

#[cfg(target_os = "linux")]
#[test]
fn a_registry_query_that_hangs_is_given_up_on() {
    // A reg.exe that never answers must not hold Settings or a launch:
    // the run gives up at its limit and kills the process it started.
    let dir = tempfile::tempdir().expect("tmp");
    let pid_file = dir.path().join("pid");
    let cmd = registry::reg_command(
        "sh",
        &strings(&[
            "-c",
            "echo $$ > \"$1\"; exec sleep 30",
            "sh",
            &pid_file.display().to_string(),
        ]),
    );
    let started = std::time::Instant::now();
    let out = registry::reg_output(cmd, std::time::Duration::from_millis(300));
    assert!(out.is_none());
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    // The process it started is gone, or a zombie waiting to be reaped:
    // killed, not left running.
    let pid = std::fs::read_to_string(&pid_file).expect("the child wrote its pid");
    let stat = format!("/proc/{}/stat", pid.trim());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let state = std::fs::read_to_string(&stat)
            .ok()
            .and_then(|s| s.rsplit(')').next().map(|r| r.trim().chars().next()))
            .flatten();
        if matches!(state, None | Some('Z')) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the hung query is still running: {state:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn a_registry_query_that_answers_gives_its_output() {
    let cmd = registry::reg_command(
        "sh",
        &strings(&[
            "-c",
            "printf '    (Default)    REG_SZ    C:\\\\VLC\\\\vlc.exe\\n'",
        ]),
    );
    let out = registry::reg_output(cmd, std::time::Duration::from_secs(10)).expect("output");
    assert!(out.status.success());
    assert_eq!(
        parse_reg_default(&String::from_utf8_lossy(&out.stdout)).as_deref(),
        Some("C:\\VLC\\vlc.exe")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_registry_query_runs_from_inside_an_async_task() {
    // The lookup may be reached from async code: running the query
    // must not try to start a runtime inside the current one.
    let cmd = registry::reg_command("sh", &strings(&["-c", "echo ok"]));
    let out = registry::reg_output(cmd, std::time::Duration::from_secs(10));
    assert!(out.is_some_and(|o| o.status.success()));
}
