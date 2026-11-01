//! Where a track stands in the preference order; split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use super::tag::{implied_script, parse_tag, Tag};
use crate::scraper::provider::SubtitleTrack;

/// The language of a track: its `lang` read as a tag, or — for a
/// provider whose `lang` is a name rather than a tag — the tag that
/// name or the label reads as.
fn track_tag(track: &SubtitleTrack) -> Option<Tag> {
    parse_tag(&track.lang).or_else(|| {
        [track.lang.as_str(), track.label.as_str()]
            .into_iter()
            .find_map(|text| crate::scraper::subtitle_language::track_tag("", text))
            .and_then(|tag| parse_tag(&tag))
    })
}

/// The script a tag is written in: the one it names, or the one its
/// region writes Chinese in.
fn script_of(tag: &Tag) -> Option<&str> {
    tag.script
        .as_deref()
        .or_else(|| tag.region.as_deref().and_then(implied_script))
}

/// How close a track is to the language `want` names, or `None` when
/// it is in another: 0 the same region, 1 the same script, 2 the
/// language alone.
fn closeness(track: &Tag, want: &Tag) -> Option<u8> {
    if track.language != want.language {
        return None;
    }
    if want.region.is_some() && track.region == want.region {
        return Some(0);
    }
    if script_of(want).is_some() && script_of(track) == script_of(want) {
        return Some(1);
    }
    Some(2)
}

/// Where a track stands: 0–2 the show's remembered language (its
/// region, its script, any), 3–5 the user's locale's the same way, 6
/// the provider's default, 7 English, 8 any other known language, 9 an
/// unknown one — the order the built-in player starts by
/// (`$lib/play/caption-start.ts`), so the first track is the one it
/// would show.
pub(super) fn rank(track: &SubtitleTrack, remembered: Option<&Tag>, user: Option<&Tag>) -> u8 {
    let tag = track_tag(track);
    if let Some(tag) = tag.as_ref() {
        if let Some(k) = remembered.and_then(|r| closeness(tag, r)) {
            return k;
        }
        if let Some(k) = user.and_then(|u| closeness(tag, u)) {
            return 3 + k;
        }
    }
    if track.default {
        return 6;
    }
    match tag {
        None => 9,
        Some(t) if t.language == "en" => 7,
        Some(_) => 8,
    }
}
