//! Whether one squashed episode title holds another, for the title
//! comparison in `super::titles`.

/// Shortest squashed title that counts as contained in another.
const MIN_CONTAINED: usize = 3;

/// A title's letters and digits with nothing between, and the byte
/// offsets in it where the title's words begin and end.
pub(super) struct Squashed {
    /// The words, joined.
    pub(super) text: String,
    /// Every offset in `text` at which a word starts or ends.
    pub(super) bounds: Vec<usize>,
}

impl Squashed {
    /// `words` joined, with their edges recorded.
    pub(super) fn of_words(words: impl Iterator<Item = String>) -> Self {
        let mut text = String::new();
        let mut bounds = vec![0];
        for word in words {
            text.push_str(&word);
            bounds.push(text.len());
        }
        Self { text, bounds }
    }
}

/// One squashed title holds the other, and the held one is at least
/// [`MIN_CONTAINED`] characters long: "go" is inside nearly anything.
/// A number the held title starts or ends with must meet a whole
/// number where it is held ([`number_edge_holds`]): "Part 1" is not
/// inside "Part 11", while "Chapter 1" is inside "Chapter 1 - 100
/// Years Later".
pub(super) fn contains_other(a: &Squashed, b: &Squashed) -> bool {
    let (long, short) = if a.text.len() >= b.text.len() {
        (a, b)
    } else {
        (b, a)
    };
    let (long_text, short_text) = (long.text.as_str(), short.text.as_str());
    short_text.chars().count() >= MIN_CONTAINED
        && (0..=long_text.len() - short_text.len())
            .filter(|at| {
                long_text.is_char_boundary(*at) && long_text[*at..].starts_with(short_text)
            })
            .any(|at| {
                let end = at + short_text.len();
                number_edge_holds(
                    long,
                    at,
                    short_text.chars().next(),
                    long_text[..at].chars().next_back(),
                ) && number_edge_holds(
                    long,
                    end,
                    short_text.chars().next_back(),
                    long_text[end..].chars().next(),
                )
            })
}

/// At offset `at` of `long`, where the held title's `edge` character
/// meets `neighbour`: a digit edge holds when the neighbour is no
/// digit, or when `long`'s words split there — the number is a word
/// of its own, whatever word follows it.
fn number_edge_holds(
    long: &Squashed,
    at: usize,
    edge: Option<char>,
    neighbour: Option<char>,
) -> bool {
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    !(digit(edge) && digit(neighbour)) || long.bounds.contains(&at)
}
