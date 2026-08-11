//! Per-entry vs provider episode numbering — split from
//! `play_native_resolve` so each file stays inside the complexity
//! ratchet's per-file bar.

/// The shift between the provider's episode numbers and the
/// per-entry numbers every caller requests. anidb.app keeps a
/// franchise's cumulative count on continuation cours — TYBW's
/// fourth part lists episodes 41 and 42 — while Kitsu (and so every
/// number the UI can send) restarts each entry at 1. An entry whose
/// first listed number is above 1 is such a continuation; entries
/// starting at 0 or 1 already number per-entry and shift nothing.
pub fn numbering_offset(episodes: &[crate::scraper::anidb::EpisodeRef]) -> u32 {
    match episodes.iter().filter_map(integer_display).min() {
        Some(first) if first > 1 => first - 1,
        _ => 0,
    }
}

/// A row's integer display identity: the integer tag when the row
/// carries one, the slot number when untagged, `None` for a
/// fractional extra. The continuation numbering can live in either
/// place — TYBW's fourth part lists slots 41..42 bare, while other
/// continuations restart slots at 1 and put 41..42 in the tags.
fn integer_display(e: &crate::scraper::anidb::EpisodeRef) -> Option<u32> {
    match e.number2.as_deref() {
        Some(tag) => tag.parse::<u32>().ok(),
        None => Some(e.number),
    }
}

/// The entry's highest listed episode in per-entry (Kitsu) numbering
/// — what availability caps and the play response's `episode_cap`
/// must report, or a continuation cour's raw provider numbers unlock
/// episodes that don't exist.
pub fn kitsu_episode_cap(episodes: &[crate::scraper::anidb::EpisodeRef]) -> Option<u32> {
    let offset = numbering_offset(episodes);
    // The display tag is the row's identity: integer tags count
    // toward the cap, fractional ones are extras and must not let a
    // recap's slot advertise an episode no request can resolve.
    episodes
        .iter()
        .filter_map(integer_display)
        .max()
        .map(|m| m.saturating_sub(offset))
}

/// The number of regular episodes a listing carries — rows whose
/// display identity is an integer. Kitsu's episode_count excludes
/// recaps, so candidate scoring must too.
pub fn regular_episode_count(episodes: &[crate::scraper::anidb::EpisodeRef]) -> u32 {
    let regular = episodes
        .iter()
        .filter(|e| match e.number2.as_deref() {
            Some(tag) => tag.parse::<u32>().is_ok(),
            None => true,
        })
        .count();
    u32::try_from(regular).unwrap_or(u32::MAX)
}

/// The provider's fractional display tags, in listing order — what
/// availability advertises as `extra_episodes`. An integer `number2`
/// is a continuation's cumulative re-display, not an extra; every
/// non-integer tag is playable verbatim through the resolve's
/// `number2` match, which is exactly why the listing outranks any
/// cached row as a source for them.
pub fn extra_episode_tags(episodes: &[crate::scraper::anidb::EpisodeRef]) -> Vec<String> {
    let offset = numbering_offset(episodes);
    episodes
        .iter()
        .filter_map(|e| e.number2.as_deref())
        .filter(|tag| tag.parse::<u32>().is_err())
        .map(|tag| per_entry_fraction(tag, offset))
        .collect()
}

/// A provider fraction in per-entry numbering: "41.5" under offset
/// 40 advertises as "1.5". Tags without a parseable integer part
/// above the offset pass through verbatim.
pub fn per_entry_fraction(tag: &str, offset: u32) -> String {
    if offset == 0 {
        // Identity, byte-for-byte: reconstruction would normalize
        // away a leading zero and break the verbatim tag match.
        return tag.to_string();
    }
    let Some((int, frac)) = tag.split_once('.') else {
        return tag.to_string();
    };
    match int.parse::<u32>() {
        Ok(n) if n > offset => format!("{}.{frac}", n - offset),
        _ => tag.to_string(),
    }
}

/// The inverse of [`per_entry_fraction`]: the provider tag a
/// per-entry fractional request names.
pub fn provider_fraction(request: &str, offset: u32) -> String {
    if offset == 0 {
        return request.to_string();
    }
    let Some((int, frac)) = request.split_once('.') else {
        return request.to_string();
    };
    match int.parse::<u32>() {
        Ok(n) => format!("{}.{frac}", n + offset),
        Err(_) => request.to_string(),
    }
}

#[cfg(test)]
#[path = "play_native_numbering_test.rs"]
mod tests;
