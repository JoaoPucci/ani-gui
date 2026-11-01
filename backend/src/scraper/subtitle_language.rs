//! The language of a subtitle track a provider lists without one.
//!
//! megaplay's listing carries a track's `file`, `label` and `kind`, and
//! no language field. Its CDNs name the files by a language code in
//! one of two patterns, and the site moved from the first to the
//! second without notice:
//!
//! - `track_<n>_<code>.vtt`, with words between that can name a
//!   region (`track_2_Latin_American_spa.vtt`);
//! - `<code>-<n>.vtt`, the code ahead of the track's place in the
//!   listing (`por-24.vtt`).
//!
//! The label names the language in English, with the region or script
//! in parentheses (`Portuguese (- Brazil)`, `Chinese (- Traditional)`,
//! `Spanish (- Spanish[LAT])`). So the language is read from either
//! source: the file name's code when it has one the table knows, or
//! else the label's name — and the region or script from the label
//! whenever the label names that same language. The result is a BCP 47
//! tag (`pt-BR`, `es-419`, `zh-Hant`, `en`), what the player's
//! `srclang` and a download's file name take. A track whose file name
//! and label both name no language the table knows is `und`, never a
//! guess: it plays with its label and no language, and the reader logs
//! it ([`super::hianime::parse_sources`]). A third file naming is read
//! from the labels, which name the language whatever the files are
//! called.

/// One language the reader recognises: its BCP 47 subtag, the
/// three-letter codes a file name can carry for it (bibliographic and
/// terminological), and the English names a label starts with.
struct Language {
    tag: &'static str,
    codes: &'static [&'static str],
    names: &'static [&'static str],
}

/// The languages a track can be read as. Longer names come first
/// where one starts another (`norwegian bokmål` before `norwegian`).
const LANGUAGES: &[Language] = &[
    Language {
        tag: "ar",
        codes: &["ara"],
        names: &["arabic"],
    },
    Language {
        tag: "bg",
        codes: &["bul"],
        names: &["bulgarian"],
    },
    Language {
        tag: "ca",
        codes: &["cat"],
        names: &["catalan"],
    },
    Language {
        tag: "cs",
        codes: &["cze", "ces"],
        names: &["czech"],
    },
    Language {
        tag: "da",
        codes: &["dan"],
        names: &["danish"],
    },
    Language {
        tag: "de",
        codes: &["ger", "deu"],
        names: &["german"],
    },
    Language {
        tag: "el",
        codes: &["gre", "ell"],
        names: &["greek"],
    },
    Language {
        tag: "en",
        codes: &["eng"],
        names: &["english"],
    },
    Language {
        tag: "es",
        codes: &["spa"],
        names: &["spanish", "castilian"],
    },
    Language {
        tag: "fa",
        codes: &["per", "fas"],
        names: &["persian"],
    },
    Language {
        tag: "fi",
        codes: &["fin"],
        names: &["finnish"],
    },
    Language {
        tag: "fil",
        codes: &["fil", "tgl"],
        names: &["filipino", "tagalog"],
    },
    Language {
        tag: "fr",
        codes: &["fre", "fra"],
        names: &["french"],
    },
    Language {
        tag: "he",
        codes: &["heb"],
        names: &["hebrew"],
    },
    Language {
        tag: "hi",
        codes: &["hin"],
        names: &["hindi"],
    },
    Language {
        tag: "hr",
        codes: &["hrv"],
        names: &["croatian"],
    },
    Language {
        tag: "hu",
        codes: &["hun"],
        names: &["hungarian"],
    },
    Language {
        tag: "id",
        codes: &["ind"],
        names: &["indonesian"],
    },
    Language {
        tag: "it",
        codes: &["ita"],
        names: &["italian"],
    },
    Language {
        tag: "ja",
        codes: &["jpn"],
        names: &["japanese"],
    },
    Language {
        tag: "ko",
        codes: &["kor"],
        names: &["korean"],
    },
    Language {
        tag: "ms",
        codes: &["may", "msa"],
        names: &["malay"],
    },
    Language {
        tag: "nb",
        codes: &["nob"],
        names: &["norwegian bokmål", "norwegian (bokmål)", "bokmål"],
    },
    Language {
        tag: "nl",
        codes: &["dut", "nld"],
        names: &["dutch"],
    },
    Language {
        tag: "no",
        codes: &["nor"],
        names: &["norwegian"],
    },
    Language {
        tag: "pl",
        codes: &["pol"],
        names: &["polish"],
    },
    Language {
        tag: "pt",
        codes: &["por"],
        names: &["portuguese"],
    },
    Language {
        tag: "ro",
        codes: &["rum", "ron"],
        names: &["romanian"],
    },
    Language {
        tag: "ru",
        codes: &["rus"],
        names: &["russian"],
    },
    Language {
        tag: "sk",
        codes: &["slo", "slk"],
        names: &["slovak"],
    },
    Language {
        tag: "sr",
        codes: &["srp"],
        names: &["serbian"],
    },
    Language {
        tag: "sv",
        codes: &["swe"],
        names: &["swedish"],
    },
    Language {
        tag: "ta",
        codes: &["tam"],
        names: &["tamil"],
    },
    Language {
        tag: "te",
        codes: &["tel"],
        names: &["telugu"],
    },
    Language {
        tag: "th",
        codes: &["tha"],
        names: &["thai"],
    },
    Language {
        tag: "tr",
        codes: &["tur"],
        names: &["turkish"],
    },
    Language {
        tag: "uk",
        codes: &["ukr"],
        names: &["ukrainian"],
    },
    Language {
        tag: "vi",
        codes: &["vie"],
        names: &["vietnamese"],
    },
    Language {
        tag: "zh",
        codes: &["chi", "zho"],
        names: &["chinese", "mandarin"],
    },
];

/// The words in a label that name a script, and its subtag.
const SCRIPTS: &[(&str, &str)] = &[("simplified", "Hans"), ("traditional", "Hant")];

/// The words in a label that name a region, and its subtag. Matched as
/// written in the labels seen: `(- Brazil)`, `(Portuguese - Brazilian)`,
/// `(- Latin America)`, `(Latin American)`, `[LAT]`, `[ESP]`.
const REGIONS: &[(&str, &str)] = &[
    ("brazil", "BR"),
    ("portugal", "PT"),
    ("latin america", "419"),
    ("[lat]", "419"),
    ("latam", "419"),
    ("spain", "ES"),
    ("[esp]", "ES"),
    ("castilian", "ES"),
    ("mexic", "MX"),
    ("canad", "CA"),
    ("france", "FR"),
    ("united kingdom", "GB"),
    ("british", "GB"),
    ("united states", "US"),
    ("american", "US"),
    ("taiwan", "TW"),
    ("hong kong", "HK"),
];

/// The language tag a file name's three-letter code names.
fn language_of_code(code: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|l| l.tag == code || l.codes.contains(&code))
        .map(|l| l.tag)
}

/// The language a label names: the English name it starts with, as a
/// whole word.
fn language_of_label(label: &str) -> Option<&'static str> {
    let label = label.trim().to_lowercase();
    LANGUAGES
        .iter()
        .find(|l| {
            l.names.iter().any(|name| {
                label
                    .strip_prefix(name)
                    .is_some_and(|rest| !rest.starts_with(char::is_alphanumeric))
            })
        })
        .map(|l| l.tag)
}

/// The script and region subtags the label's words name, the first in
/// table order of each. Whether they apply to the track is
/// [`track_tag`]'s call.
fn qualifiers_of_label(label: &str) -> (Option<&'static str>, Option<&'static str>) {
    let label = label.to_lowercase();
    let script = SCRIPTS
        .iter()
        .find(|(word, _)| label.contains(word))
        .map(|(_, subtag)| *subtag);
    let region = REGIONS
        .iter()
        .find(|(word, _)| label.contains(word))
        .map(|(_, subtag)| *subtag);
    (script, region)
}

/// The code a file name carries, in either of the CDNs' namings: the
/// three lowercase letters at the end of its stem, after an underscore
/// or alone, once a trailing `-<digits>` place is set aside.
fn code_of_file(file: &str) -> Option<&str> {
    let name = file
        .rsplit('/')
        .next()
        .unwrap_or(file)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let stem = name
        .strip_suffix(".vtt")
        .or_else(|| name.strip_suffix(".VTT"))
        .unwrap_or(name);
    let stem = match stem.rsplit_once('-') {
        Some((head, place)) if !place.is_empty() && place.bytes().all(|b| b.is_ascii_digit()) => {
            head
        }
        _ => stem,
    };
    let code = stem.rsplit('_').next().unwrap_or_default();
    (code.len() == 3 && code.bytes().all(|b| b.is_ascii_lowercase())).then_some(code)
}

/// The BCP 47 tag of a track listed as `file` under `label`, or `None`
/// when neither names a language the table knows. The language is the
/// file name's code when the table knows it, the label's name
/// otherwise; the script and region come from the label, and only
/// when the label names the same language — a label for another
/// language says nothing about this one's region. Whatever else the
/// label carries (`SDH`) stays in the label, which is what the player
/// shows.
#[must_use]
pub fn track_tag(file: &str, label: &str) -> Option<String> {
    let from_label = language_of_label(label);
    let language = code_of_file(file)
        .and_then(language_of_code)
        .or(from_label)?;
    let mut tag = language.to_string();
    if from_label == Some(language) {
        let (script, region) = qualifiers_of_label(label);
        if let Some(script) = script {
            tag.push('-');
            tag.push_str(script);
        }
        // The United States is a region only for English: a Spanish
        // label calling itself `American` is read as Spanish, no region
        // (`Latin American` already matched `latin america` first).
        if let Some(region) = region.filter(|r| *r != "US" || language == "en") {
            tag.push('-');
            tag.push_str(region);
        }
    }
    Some(tag)
}
