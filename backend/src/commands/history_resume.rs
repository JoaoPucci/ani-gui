//! Which of two history rows for one show a Kitsu entry resumes from,
//! by the rule the Continue Watching strip applies to the same rows,
//! so the two surfaces name one episode.

use std::cmp::Ordering;

/// Whether `candidate` is the row to resume from over `current`: a
/// later watched-at stamp, or a stamp where the other has none — a
/// stamp is the record of a watch, and a row without one never
/// reached the watch record or predates it — and, when the stamps
/// do not separate the two, the further progress. Equal on every
/// count is not later, so the row met first keeps its place.
#[must_use]
pub(crate) fn resumes_over(candidate: (Option<i64>, &str), current: (Option<i64>, &str)) -> bool {
    match candidate.0.cmp(&current.0) {
        Ordering::Equal => {
            progress_of(candidate.1).total_cmp(&progress_of(current.1)) == Ordering::Greater
        }
        ordering => ordering == Ordering::Greater,
    }
}

/// The progress an episode number stands for, read the way the
/// strip reads it: the number the text begins with, so the `12.5`
/// recap is more progress than `12`; text that begins with no
/// number — a user-edited or otherwise malformed row — is below any
/// real episode, so it never beats a good row.
#[must_use]
pub(crate) fn progress_of(ep_no: &str) -> f64 {
    let s = ep_no.trim_start();
    let digits = |from: usize| s[from..].bytes().take_while(u8::is_ascii_digit).count();
    let mut end = usize::from(s.starts_with(['+', '-']));
    let whole = digits(end);
    end += whole;
    let mut fraction = 0;
    if s[end..].starts_with('.') {
        fraction = digits(end + 1);
        end += 1 + fraction;
    }
    if whole == 0 && fraction == 0 {
        return -1.0;
    }
    if s[end..].starts_with(['e', 'E']) {
        let mut exponent = end + 1;
        if s[exponent..].starts_with(['+', '-']) {
            exponent += 1;
        }
        let exponent_digits = digits(exponent);
        if exponent_digits > 0 {
            end = exponent + exponent_digits;
        }
    }
    s[..end]
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .unwrap_or(-1.0)
}

#[cfg(test)]
#[path = "history_resume_test.rs"]
mod tests;
