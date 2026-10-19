//! A title without its catalogue tags — the release years and format
//! tags the provider brackets onto it — which every reader of a title
//! reads it without. Split from `play_native_title_grammar` for the
//! per-file complexity bar.

/// `title` lowercased, without the bracketed release years and format
/// tags the provider adds to a title — a four-digit
/// year ("Gintama (2011)") or TV, ONA, OVA or OAD ("JoJo no Kimyou na
/// Bouken (TV)", "[OVA]") in round, square or lenticular brackets,
/// brackets and tag alike ASCII or full-width ("（ＴＶ）", "［ＯＶＡ］"),
/// each character of the tag read in its ASCII form on its own, so
/// "(ｔv)" is the tag too. Other bracketed words are left as they are.
pub(crate) fn untagged(title: &str) -> String {
    split_tags(title).0
}

/// `title` lowercased and split into its text without catalogue tags
/// ([`untagged`]) and the tags it carries, each read in its ASCII form
/// ("2011", "tv"), sorted: what a comparison of two titles as written
/// reads, wherever the tags stand.
pub(crate) fn split_tags(title: &str) -> (String, Vec<String>) {
    let text = title.to_lowercase();
    let mut out = String::new();
    let mut tags = Vec::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find(['(', '[', '（', '［', '【']) {
        let after = &rest[open..];
        let inner_start = after.chars().next().map_or(1, char::len_utf8);
        let tag = after.find([')', ']', '）', '］', '】']).and_then(|close| {
            let inner: String = after[inner_start..close]
                .trim()
                .chars()
                .map(ascii_form)
                .collect();
            let is_tag = (inner.len() == 4 && inner.chars().all(|c| c.is_ascii_digit()))
                || ["tv", "ona", "ova", "oad"].contains(&inner.as_str());
            is_tag.then_some((close, inner))
        });
        let Some((close, inner)) = tag else {
            out.push_str(&rest[..open + inner_start]);
            rest = &rest[open + inner_start..];
            continue;
        };
        out.push_str(&rest[..open]);
        tags.push(inner);
        let closer = after[close..].chars().next().map_or(1, char::len_utf8);
        rest = &after[close + closer..];
    }
    out.push_str(rest);
    tags.sort();
    (out, tags)
}

/// A full-width form's ASCII letter, digit or mark, lowercased; any
/// other character as it is.
pub(crate) fn ascii_form(c: char) -> char {
    match c {
        '！'..='～' => char::from_u32(c as u32 - 0xFEE0).map_or(c, |a| a.to_ascii_lowercase()),
        _ => c,
    }
}
