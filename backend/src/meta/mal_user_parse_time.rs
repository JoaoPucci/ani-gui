//! MAL's ISO-8601 timestamps as Unix seconds ([`super`]); split from
//! the list-status parsers so each file stays inside the CRAP gate's
//! per-file bar.

/// Minimal RFC 3339 / ISO 8601 parser. MAL always emits the canonical
/// `YYYY-MM-DDTHH:MM:SS±HH:MM` (or trailing `Z`) shape — we extract
/// the date + time numerically and normalise the trailing offset to
/// UTC. The offset used to be ignored — safe only while these values
/// were compared with each other, since every row carried the same
/// skew — but the Watch Later rail now sorts them against AniList's
/// true Unix seconds.
///
/// Returns 0 for unparseable input so a malformed row doesn't fail
/// the whole list page.
pub(in crate::meta) fn parse_iso8601_to_epoch(s: &str) -> i64 {
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return 0;
    }
    let parse_u = |start: usize, end: usize| -> Option<i64> {
        std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
    };
    let Some(y) = parse_u(0, 4) else { return 0 };
    let Some(m) = parse_u(5, 7) else { return 0 };
    let Some(d) = parse_u(8, 10) else { return 0 };
    let Some(hh) = parse_u(11, 13) else { return 0 };
    let Some(mm) = parse_u(14, 16) else { return 0 };
    let Some(ss) = parse_u(17, 19) else { return 0 };
    // Howard Hinnant's days_from_civil algorithm — exact for all
    // years in the proleptic Gregorian calendar.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let local = days * 86400 + hh * 3600 + mm * 60 + ss;
    local - utc_offset_secs(bytes)
}

/// Seconds to subtract to turn the wall clock into UTC. `Z`, a
/// missing suffix, or anything malformed all mean "already UTC" —
/// a row whose offset cannot be read is better placed by its wall
/// clock than dropped from the list.
fn utc_offset_secs(bytes: &[u8]) -> i64 {
    let sign = match bytes.get(19) {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return 0,
    };
    if bytes.len() < 25 {
        return 0;
    }
    let num = |start: usize, end: usize| -> Option<i64> {
        std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
    };
    let (Some(oh), Some(om)) = (num(20, 22), num(23, 25)) else {
        return 0;
    };
    sign * (oh * 3600 + om * 60)
}
