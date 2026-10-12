//! A title without its catalogue tags — the release years and format
//! tags the provider brackets onto it — which every reader of a title
//! reads it without. Split from `play_native_title_grammar` for the
//! per-file complexity bar.

/// `title` lowercased, without the bracketed release years and format
/// tags the provider adds to a title — a four-digit
/// year ("Gintama (2011)") or TV, ONA, OVA or OAD ("JoJo no Kimyou na
/// Bouken (TV)", "[OVA]") in round, square or lenticular brackets,
/// brackets and tag alike ASCII or full-width ("（ＴＶ）"). Other
/// bracketed words are left as they are.
pub(crate) fn untagged(title: &str) -> String {
    let text = title.to_lowercase();
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find(['(', '[', '（', '【']) {
        let after = &rest[open..];
        let inner_start = after.chars().next().map_or(1, char::len_utf8);
        let tag_end = after.find([')', ']', '）', '】']).filter(|&close| {
            let inner: String = after[inner_start..close]
                .trim()
                .chars()
                .map(ascii_form)
                .collect();
            (inner.len() == 4 && inner.chars().all(|c| c.is_ascii_digit()))
                || ["tv", "ona", "ova", "oad"].contains(&inner.as_str())
        });
        let Some(close) = tag_end else {
            out.push_str(&rest[..open + inner_start]);
            rest = &rest[open + inner_start..];
            continue;
        };
        out.push_str(&rest[..open]);
        let closer = after[close..].chars().next().map_or(1, char::len_utf8);
        rest = &after[close + closer..];
    }
    out.push_str(rest);
    out
}

/// A full-width form's ASCII letter, digit or mark, lowercased; any
/// other character as it is.
fn ascii_form(c: char) -> char {
    match c {
        '！'..='～' => char::from_u32(c as u32 - 0xFEE0).map_or(c, |a| a.to_ascii_lowercase()),
        _ => c,
    }
}
