//! The name an episode's download takes: the title's file-safe
//! spelling made visible and not a device, or the name an earlier
//! version gave the episode when that file is already in the folder —
//! split from `download_names` for the per-file complexity bar.

/// The name an episode's download takes in `dest`: `"{title} Episode
/// {episode}"` with the title made
/// [`file_safe`](super::download_names::file_safe), [`unhidden`] and
/// not a device, or just `"Episode {episode}"` for a title of dots.
/// Such a title has no show folder either, so every title of dots
/// names its episodes alike in the chosen folder: nothing visible is
/// left of it to tell them apart, and a name that kept the dots would
/// be hidden.
///
/// When a file is already there under a name an earlier version gave
/// the episode, that file is the episode and keeps the name the user
/// has it under. The earlier names are the ones numbered `former`, the
/// ones spelled with the title's leading dots, the ones from before
/// the device rule, and the title as the provider gave it. The name
/// this version gives leads each number's spellings, so where the
/// visible and the dotted file are both there for one number the
/// visible one is it. The former number is looked for before the
/// episode's own, as it always was.
///
/// The spelling from before the device rule is looked for only off
/// Windows, which opens it as the device. The provider's spelling is
/// too: earlier versions saved it as a real file only where the
/// filesystem accepts the colon, and on Windows the same path names a
/// stream of the empty file a failed download left, so finding that
/// stream would hand the broken name out again.
#[must_use]
pub(crate) fn episode_stem(
    dest: &std::path::Path,
    title: &str,
    episode: &str,
    former: Option<&str>,
) -> String {
    let safe = super::download_names::file_safe(title);
    let shown = unhidden(&safe);
    let dotted = shown != safe;
    let stem = |t: &str, n: &str| format!("{t} Episode {n}");
    let plain = |n: &str| {
        if dotted && shown.is_empty() {
            format!("Episode {n}")
        } else {
            stem(shown, n)
        }
    };
    let named = |n: &str| not_a_device(plain(n));
    let on_disk = |s: &String| std::fs::symlink_metadata(dest.join(format!("{s}.mp4"))).is_ok();
    // A title with a slash was never an earlier download's name, and
    // joining it as a path can reach outside `dest`.
    let raw = !cfg!(windows) && safe != title && !title.contains('/');
    let mut earlier: Vec<String> = Vec::new();
    for n in former.into_iter().chain([episode]) {
        earlier.push(named(n));
        // Before the device rule, an undotted title's name was this
        // one with its dot: an ordinary file off Windows, and a device
        // on it.
        if !cfg!(windows) && plain(n) != named(n) {
            earlier.push(plain(n));
        }
        if dotted {
            earlier.push(stem(&safe, n));
        }
        if raw {
            earlier.push(stem(title, n));
        }
    }
    earlier
        .into_iter()
        .find(on_disk)
        .unwrap_or_else(|| named(episode))
}

/// `name` without the dots it starts with, and the spaces between and
/// after them: Linux and macOS hide a file whose name starts with a
/// dot, and the user could not see the episode they downloaded
/// (`.hack//Sign`). A name that does not start with a dot was never
/// hidden and is returned as it is, so its episodes keep the names
/// they have.
#[must_use]
fn unhidden(name: &str) -> &str {
    if name.starts_with('.') {
        name.trim_start_matches(|c: char| c == '.' || c.is_whitespace())
    } else {
        name
    }
}

/// `name` with its first dot made a hyphen when the part before that
/// dot is a device name, which Windows 10 opens as the device whatever
/// follows the dot. Any other name is returned as it is.
pub(in crate::commands) fn not_a_device(name: String) -> String {
    const DEVICES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let Some(dot) = name.find('.') else {
        return name;
    };
    let base = name[..dot].trim_end().to_ascii_uppercase();
    let numbered = |prefix: &str| {
        base.strip_prefix(prefix).is_some_and(|n| {
            let mut digits = n.chars();
            matches!(
                (digits.next(), digits.next()),
                (Some('1'..='9' | '¹' | '²' | '³'), None)
            )
        })
    };
    if DEVICES.contains(&base.as_str()) || numbered("COM") || numbered("LPT") {
        let mut renamed = name;
        renamed.replace_range(dot..=dot, "-");
        renamed
    } else {
        name
    }
}
