//! Whether one squashed episode title holds another, for the title
//! comparison in `super::titles`.

/// Shortest squashed title that counts as contained in another.
const MIN_CONTAINED: usize = 3;

/// One squashed title holds the other, and the held one is at least
/// [`MIN_CONTAINED`] characters long: "go" is inside nearly anything.
/// A number the held title starts or ends with must meet a whole
/// number where it is held: "arcpart1" is not inside "arcpart11".
pub(super) fn contains_other(a: &str, b: &str) -> bool {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    short.chars().count() >= MIN_CONTAINED
        && (0..=long.len() - short.len())
            .filter(|at| long.is_char_boundary(*at) && long[*at..].starts_with(short))
            .any(|at| numbers_stay_whole(long, at, short))
}

/// `short`, found in `long` at byte `at`, neither continues a number
/// before it nor is continued by one after it.
fn numbers_stay_whole(long: &str, at: usize, short: &str) -> bool {
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    let before = long[..at].chars().next_back();
    let after = long[at + short.len()..].chars().next();
    let runs_on_before = digit(short.chars().next()) && digit(before);
    let runs_on_after = digit(short.chars().next_back()) && digit(after);
    !(runs_on_before || runs_on_after)
}
