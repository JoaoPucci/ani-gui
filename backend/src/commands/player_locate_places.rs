//! Where each OS's installers put mpv, VLC and IINA, outside PATH.
//! Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::{join, Os};

/// The places `os`'s installers put `name`, outside PATH. A place
/// under a variable that is unset is skipped.
pub(super) fn installs(name: &str, os: Os, var: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let under = |v: &str, rest: &str| var(v).map(|base| join(&base, sep_of(os), rest));
    let fixed = |paths: &[&str]| paths.iter().map(|p| Some((*p).to_string())).collect();
    let found: Vec<Option<String>> = match (os, name) {
        (Os::Windows, "vlc") => vec![
            under("ProgramFiles", "VideoLAN\\VLC\\vlc.exe"),
            under("ProgramFiles(x86)", "VideoLAN\\VLC\\vlc.exe"),
        ],
        (Os::Windows, "mpv") => vec![
            under("ProgramFiles", "mpv\\mpv.exe"),
            under("ProgramFiles(x86)", "mpv\\mpv.exe"),
            under("LOCALAPPDATA", "Programs\\mpv\\mpv.exe"),
            under("USERPROFILE", "scoop\\shims\\mpv.exe"),
            under("LOCALAPPDATA", "Microsoft\\WinGet\\Links\\mpv.exe"),
        ],
        (Os::Linux, "vlc") => flatpak_and_snap("org.videolan.VLC", "vlc", var),
        (Os::Linux, "mpv") => flatpak_and_snap("io.mpv.Mpv", "mpv", var),
        (Os::MacOs, "iina") => bundles("IINA.app/Contents/MacOS/iina-cli", var),
        (Os::MacOs, "vlc") => bundles("VLC.app/Contents/MacOS/VLC", var),
        (Os::MacOs, "mpv") => {
            let mut v = bundles("mpv.app/Contents/MacOS/mpv", var);
            v.extend::<Vec<Option<String>>>(fixed(&[
                "/opt/homebrew/bin/mpv",
                "/usr/local/bin/mpv",
            ]));
            v
        }
        _ => Vec::new(),
    };
    found.into_iter().flatten().collect()
}

fn sep_of(os: Os) -> char {
    match os {
        Os::Windows => '\\',
        Os::Linux | Os::MacOs => '/',
    }
}

/// A Linux program's flatpak launchers (system, then per-user) and its
/// snap launcher.
fn flatpak_and_snap(
    app_id: &str,
    snap: &str,
    var: &dyn Fn(&str) -> Option<String>,
) -> Vec<Option<String>> {
    vec![
        Some(format!("/var/lib/flatpak/exports/bin/{app_id}")),
        var("HOME").map(|h| {
            format!(
                "{}/.local/share/flatpak/exports/bin/{app_id}",
                h.trim_end_matches('/')
            )
        }),
        Some(format!("/snap/bin/{snap}")),
    ]
}

/// A macOS app's program inside its bundle, in /Applications and in
/// the user's own Applications folder.
fn bundles(inner: &str, var: &dyn Fn(&str) -> Option<String>) -> Vec<Option<String>> {
    vec![
        Some(format!("/Applications/{inner}")),
        var("HOME").map(|h| format!("{}/Applications/{inner}", h.trim_end_matches('/'))),
    ]
}
