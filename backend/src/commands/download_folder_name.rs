//! The name a show's download folder takes: the title, file-safe, as one
//! path component every packaged platform creates as spelled — split
//! from `download_place` for the per-file complexity bar.

/// The longest a show's folder name is, in UTF-16 code units — the unit
/// Windows measures names and paths in.
///
/// The folder adds its name to every path under it, and the episode's
/// own name already carries the same title, so a long light-novel title
/// appears twice in each episode's path. The bound keeps that second
/// copy readable and short. It matters most for the paths the bundled
/// yt-dlp and ffmpeg are handed: they write to a scratch name in the
/// show's folder, and are not guaranteed to reach past Windows' 260-unit
/// `MAX_PATH` where the system has not opted into long paths, so the
/// folder's length is most of what stands between a deep download
/// folder and that limit. It does not bound the episode's own name,
/// which the app writes itself, through paths the standard library
/// passes on without the limit.
pub(crate) const SHOW_FOLDER_MAX_UNITS: usize = 80;

/// The names Windows opens as a device whatever follows them: a folder
/// called `CON` cannot be created at all, and `CON.x` is the console.
const DEVICES: [&str; 6] = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];

/// The folder a show's downloads go in, or `None` when its title leaves
/// no name to give one.
///
/// It is the title made [`file_safe`](super::download_names::file_safe),
/// then:
///
/// - **One space between words.** A run of whitespace reads as one
///   space: the first in it that is not an ASCII space, or an ASCII
///   space when all are. Removing a version tag leaves an ASCII space
///   where the tag stood, beside the title's own spaces — "… (Uncensored)
///   Season 2" — so the copy reads as the plain title does, its folder
///   the same, and a title's ideographic or no-break space stays the
///   title's.
/// - **No leading dot.** A folder whose name starts with one is hidden
///   on Linux and macOS, and every episode would be hidden with it
///   (`.hack//Sign`).
/// - **No trailing dot or space**, and no leading space. Windows drops
///   them from a name when it creates the folder, so the folder made
///   and the path the app goes on to use would be two spellings of
///   it — the second one wrong whenever the path is long enough for
///   the system to pass it through untouched.
/// - **At most [`SHOW_FOLDER_MAX_UNITS`]**, cut on a character, then
///   trimmed again.
/// - **Not a device name.** The part before the first dot, ignoring
///   case and trailing spaces, is not one of Windows' devices: a
///   device's name gets a hyphen in place of that dot, or after it
///   when there is no dot.
///
/// What is left being empty, `.` or `..` — a title of dots, or of
/// characters Windows refuses — gives `None`, and the episode goes in
/// the chosen folder itself, as it did before show folders, rather
/// than under a name invented for it.
#[must_use]
pub(crate) fn show_folder_name(title: &str) -> Option<String> {
    let safe = super::download_names::file_safe(title);
    let spaced = one_space(&safe);
    let trimmed = trim_edges(&spaced);
    let mut name = String::with_capacity(trimmed.len());
    let mut units = 0;
    for c in trimmed.chars() {
        units += c.len_utf16();
        if units > SHOW_FOLDER_MAX_UNITS {
            break;
        }
        name.push(c);
    }
    let name = trim_edges(&name);
    if name.is_empty() {
        return None;
    }
    Some(not_a_device(name))
}

/// `s` with each run of whitespace read as one space, as
/// [`show_folder_name`] says.
fn one_space(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut after_space = false;
    for c in s.chars() {
        let space = c.is_whitespace();
        if !(space && after_space) {
            out.push(c);
        } else if out.ends_with(' ') && c != ' ' {
            out.pop();
            out.push(c);
        }
        after_space = space;
    }
    out
}

/// `s` without leading dots and whitespace, and without trailing dots
/// and whitespace.
fn trim_edges(s: &str) -> &str {
    s.trim_start_matches(|c: char| c == '.' || c.is_whitespace())
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
}

/// `name` made not to be a device, as [`show_folder_name`] says.
fn not_a_device(name: &str) -> String {
    let (base, rest) = match name.find('.') {
        Some(dot) => (&name[..dot], Some(&name[dot + 1..])),
        None => (name, None),
    };
    if !is_device(base.trim_end()) {
        return name.to_string();
    }
    match rest {
        Some(rest) => format!("{base}-{rest}"),
        None => format!("{name}-"),
    }
}

fn is_device(base: &str) -> bool {
    let upper = base.to_uppercase();
    let numbered = |prefix: &str| {
        upper.strip_prefix(prefix).is_some_and(|n| {
            let mut digits = n.chars();
            matches!(
                (digits.next(), digits.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    };
    DEVICES.contains(&upper.as_str()) || numbered("COM") || numbered("LPT")
}
