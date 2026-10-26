//! Models the episode map's properties compare against, written
//! without the calendar module, the dates rule or the chain rule: a
//! date read by its own byte layout, month table and leap rule; days
//! counted by whole years and months, not the civil-days formula the
//! code counts with; and a chain found as the shortest prefix of the
//! tracker's hops whose counts reach Kitsu's, summed in `u64`. A
//! property that holds the code to one of these is not the code
//! restated.

use std::collections::BTreeMap;

use proptest::prelude::*;

use crate::account::episode_map::{Evidence, Segment, TrackerShow};

/// A full date as `(year, month, day)`.
pub(crate) type Ymd = (i32, u32, u32);

/// Whether `year` is a Gregorian leap year.
fn leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// How many days `month` (1–12) of `year` has.
pub(crate) fn month_length(year: i32, month: u32) -> u32 {
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    usize::try_from(month)
        .ok()
        .and_then(|m| m.checked_sub(1))
        .and_then(|i| lengths.get(i).copied())
        .unwrap_or(0)
}

/// `text` read as `(year, month, day)` where it is exactly `YYYY-MM-DD`
/// or `YYYY-MM`, ASCII digits but the dashes, a month 1–12 and a day the
/// month has; `None` for anything else.
pub(crate) fn read(text: &str) -> Option<(i32, u32, Option<u32>)> {
    let b = text.as_bytes();
    let digit = |i: usize| b[i].is_ascii_digit();
    let number =
        |r: std::ops::Range<usize>| b[r].iter().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
    if !(b.len() == 7 || b.len() == 10) || !(0..4).all(digit) || b[4] != b'-' || !(5..7).all(digit)
    {
        return None;
    }
    let year = i32::try_from(number(0..4)).ok()?;
    let month = number(5..7);
    if !(1..=12).contains(&month) {
        return None;
    }
    if b.len() == 7 {
        return Some((year, month, None));
    }
    if b[7] != b'-' || !(8..10).all(digit) {
        return None;
    }
    let day = number(8..10);
    (1..=month_length(year, month))
        .contains(&day)
        .then_some((year, month, Some(day)))
}

/// Whether `text` is a real calendar date or month.
pub(crate) fn is_real_date(text: &str) -> bool {
    read(text).is_some()
}

/// The `(year, month)` of a real date or month; `None` for anything
/// else and for no date.
pub(crate) fn month_of(text: Option<&str>) -> Option<(i32, u32)> {
    read(text?).map(|(y, m, _)| (y, m))
}

/// A full real date as `(year, month, day)`; `None` for a month alone,
/// anything unreal, and no date.
pub(crate) fn full_date(text: Option<&str>) -> Option<Ymd> {
    let (y, m, d) = read(text?)?;
    Some((y, m, d?))
}

/// `YYYY-MM-DD` of a full date.
pub(crate) fn text((y, m, d): Ymd) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// Leap years in `0..year`, for `year >= 0`.
fn leaps_before(year: i64) -> i64 {
    (year + 3) / 4 - (year + 99) / 100 + (year + 399) / 400
}

/// Days since 0000-01-01 (that day being 1), counted by whole years
/// and then month by month — independent of the civil-days formula the
/// code counts with.
pub(crate) fn ordinal((year, month, day): Ymd) -> i64 {
    let y = i64::from(year);
    let months: i64 = (1..month).map(|m| i64::from(month_length(year, m))).sum();
    365 * y + leaps_before(y) + months + i64::from(day)
}

/// The real date one day after `date`.
pub(crate) fn next_day((year, month, day): Ymd) -> Ymd {
    if day < month_length(year, month) {
        (year, month, day + 1)
    } else if month < 12 {
        (year, month + 1, 1)
    } else {
        (year + 1, 1, 1)
    }
}

/// A real full date with its year in `years`, any day of its month.
pub(crate) fn real_date_in(years: std::ops::RangeInclusive<i32>) -> impl Strategy<Value = Ymd> {
    (years, 1u32..=12).prop_flat_map(|(y, m)| (Just(y), Just(m), 1..=month_length(y, m)))
}

/// A real full date with its year in `years` within three days of its
/// month's first or last day, where the slack across a month boundary
/// decides.
pub(crate) fn edge_date_in(years: std::ops::RangeInclusive<i32>) -> impl Strategy<Value = Ymd> {
    (years, 1u32..=12, 0u32..6).prop_map(|(y, m, k)| {
        let len = month_length(y, m);
        (y, m, if k < 3 { 1 + k } else { len + 3 - k })
    })
}

/// Whether `later` is a full date at most two days after the full date
/// `earlier` — or any time before it.
fn within_two_days(earlier: Option<&str>, later: Option<&str>) -> bool {
    match (full_date(earlier), full_date(later)) {
        (Some(a), Some(b)) => ordinal(b) - ordinal(a) <= 2,
        _ => false,
    }
}

/// Kitsu's run, its own episode list's dates, and one tracker run filed
/// as Kitsu's `from..=to`: what decides whether the run is inside.
pub(crate) struct Run<'a> {
    pub(crate) kitsu_start: Option<&'a str>,
    pub(crate) kitsu_end: Option<&'a str>,
    pub(crate) start: Option<&'a str>,
    pub(crate) end: Option<&'a str>,
    pub(crate) from: u32,
    pub(crate) to: u32,
    pub(crate) airdates: &'a BTreeMap<u32, String>,
}

/// Whether a run lies inside Kitsu's, as `docs/title-resolution.md`
/// states the chain rule: all four dates real dates or months; the
/// start neither before Kitsu's start by month (or at most two days
/// before it) nor after Kitsu's end by month, the end neither after
/// Kitsu's end by month (or at most two days after it) nor before
/// Kitsu's start by month; and a bound that is not inside placed by
/// Kitsu's own episode — the segment's first for the start, its last
/// for the end — dated in the bound's month.
pub(crate) fn run_inside(run: &Run<'_>) -> bool {
    let (Some(ks), Some(ke), Some(s), Some(e)) = (
        month_of(run.kitsu_start),
        month_of(run.kitsu_end),
        month_of(run.start),
        month_of(run.end),
    ) else {
        return false;
    };
    let start_inside = (s >= ks || within_two_days(run.start, run.kitsu_start)) && s <= ke;
    let end_inside = (e <= ke || within_two_days(run.kitsu_end, run.end)) && e >= ks;
    let placed = |n: u32, month: (i32, u32)| {
        month_of(run.airdates.get(&n).map(String::as_str)) == Some(month)
    };
    (start_inside || placed(run.from, s)) && (end_inside || placed(run.to, e))
}

/// One link of an exact chain: the entry, its count, and the Kitsu
/// episodes it takes.
pub(crate) struct Link<'a> {
    pub(crate) show: &'a TrackerShow,
    pub(crate) count: u32,
    pub(crate) from: u32,
    pub(crate) to: u32,
}

/// The exact chain by its counts: the entry and the shortest prefix of
/// at least one hop such that every hop in it has a known, positive
/// count and the entry's total plus theirs reaches Kitsu's count; the
/// chain is exact when that sum is Kitsu's count, summed in `u64`, and
/// no hop in it is an entry a Kitsu entry claims. `None` otherwise.
pub(crate) fn exact_chain_model(ev: &Evidence, count: u32, total: u32) -> Option<Vec<Link<'_>>> {
    let target = u64::from(count);
    // The sum before each hop, then after the last: `None` from the
    // first hop whose count is unknown or zero onwards.
    let mut sums = vec![Some(u64::from(total))];
    for hop in &ev.chain {
        let next = sums
            .last()
            .copied()
            .flatten()
            .zip(hop.episodes.filter(|n| *n > 0))
            .map(|(s, n)| s + u64::from(n));
        sums.push(next);
    }
    let k = sums.iter().position(|s| s.is_none_or(|s| s >= target))?;
    if k == 0 || sums[k] != Some(target) {
        return None;
    }
    let hops = &ev.chain[..k];
    if hops.iter().any(|h| ev.claimed.contains(&h.id)) {
        return None;
    }
    let mut links = vec![Link {
        show: &ev.entry,
        count: total,
        from: 1,
        to: total,
    }];
    for (i, hop) in hops.iter().enumerate() {
        let before = u32::try_from(sums[i]?).ok()?;
        let after = u32::try_from(sums[i + 1]?).ok()?;
        links.push(Link {
            show: hop,
            count: after - before,
            from: before + 1,
            to: after,
        });
    }
    Some(links)
}

/// The segment a link files.
pub(crate) fn segment_of(link: &Link<'_>) -> Segment {
    Segment {
        entry: link.show.id,
        entry_total: link.count,
        entry_finished: link.show.finished,
        kitsu_from: link.from,
        kitsu_to: link.to,
        lead: 0,
        extras_after: Vec::new(),
    }
}

#[test]
fn the_model_counts_leap_years_one_by_one() {
    let mut leaps = 0;
    for year in 0..=2500i32 {
        assert_eq!(leaps_before(i64::from(year)), leaps, "{year}");
        leaps += i64::from(leap(year));
    }
}

proptest! {
    /// The model's day count steps by one from each real date to the
    /// next, across months, years and leap days.
    #[test]
    fn the_model_counts_one_day_per_day(date in real_date_in(0..=9998)) {
        prop_assert_eq!(ordinal(next_day(date)) - ordinal(date), 1);
        prop_assert!(is_real_date(&text(date)));
        prop_assert!(is_real_date(&text(next_day(date))));
    }
}
