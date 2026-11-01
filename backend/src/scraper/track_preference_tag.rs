//! Reading a track's `lang`, and the user's locale, as the parts of a
//! BCP 47 tag the preference order compares; split from [`super`] so
//! each file stays inside the CRAP gate's per-file bar.

/// A BCP 47 tag as the order reads it: its language, script and
/// region subtags, lowercased.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Tag {
    pub(super) language: String,
    pub(super) script: Option<String>,
    pub(super) region: Option<String>,
}

/// The parts of `tag`, or `None` for one with no language subtag of
/// two or three letters — `und` included, which names none. A
/// three-letter code a file name could carry (`eng`, `por`) reads as
/// the language it names.
pub(super) fn parse_tag(tag: &str) -> Option<Tag> {
    let lower = tag.trim().to_ascii_lowercase();
    let mut subtags = lower.split(['-', '_']);
    let language = subtags.next()?;
    if !is_language_subtag(language) {
        return None;
    }
    let language =
        crate::scraper::subtitle_language::language_of_code(language).unwrap_or(language);
    let mut parsed = Tag {
        language: language.to_string(),
        ..Tag::default()
    };
    for subtag in subtags {
        read_subtag(&mut parsed, subtag);
    }
    Some(parsed)
}

/// Whether `s` is a language subtag the order can compare: two or
/// three letters, and not `und`.
fn is_language_subtag(s: &str) -> bool {
    (2..=3).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase()) && s != "und"
}

/// A later subtag into `parsed`: the first four-letter one before any
/// region is the script; the first of two letters or three digits is
/// the region; anything else is passed over.
fn read_subtag(parsed: &mut Tag, subtag: &str) {
    let letters = subtag.bytes().all(|b| b.is_ascii_alphabetic());
    let digits = subtag.bytes().all(|b| b.is_ascii_digit());
    let free = parsed.region.is_none();
    match subtag.len() {
        4 if letters && free && parsed.script.is_none() => parsed.script = Some(subtag.to_string()),
        2 if letters && free => parsed.region = Some(subtag.to_string()),
        3 if digits && free => parsed.region = Some(subtag.to_string()),
        _ => {}
    }
}

/// The script a region writes Chinese in, so a `zh-TW` user is taken
/// to want the `zh-Hant` track: the listing names the script, not the
/// region.
pub(super) fn implied_script(region: &str) -> Option<&'static str> {
    match region {
        "tw" | "hk" | "mo" => Some("hant"),
        "cn" | "sg" => Some("hans"),
        _ => None,
    }
}
