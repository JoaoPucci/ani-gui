//! A show id that says whose it is, and the words a provider's slug
//! carries — split from the provider seam so each file stays inside
//! the complexity ratchet's per-file bar. Re-exported from
//! [`crate::scraper::provider`], where every caller names them.

use super::provider::ProviderId;

/// The digits after a slug's last hyphen, when there are any —
/// the entry id both providers key their listings on.
#[must_use]
pub fn slug_numeric_id(slug: &str) -> Option<u64> {
    slug.rsplit('-').next()?.parse().ok()
}

/// The Kitsu-searchable text a slug carries: its hyphenated
/// words with the trailing numeric id removed (`one-piece-69` →
/// `one piece`). `None` when `slug` isn't slug-shaped — legacy
/// allanime ids are mixed-case and hyphenless, so they fall through
/// to their own resolve path.
#[must_use]
pub fn slug_search_term(slug: &str) -> Option<String> {
    Some(slug_words(slug)?.replace('-', " "))
}

/// The slug without its numeric tail — `words-id` less the id — when
/// it is slug-shaped: the hyphenated words that name the show, which
/// the search term is made from and the cour is read off, since the
/// tail is the provider's id and would otherwise read as a trailing
/// number.
#[must_use]
pub fn slug_words(slug: &str) -> Option<&str> {
    slug_numeric_id(slug)?;
    let (words, _id) = slug.rsplit_once('-')?;
    if words.is_empty() {
        return None;
    }
    Some(words)
}

/// A show id that says whose id it is. Every store stamped by a
/// resolve — history rows, the numbering sidecar, the watched-at
/// stamps, the reverse mapping, the cache row — keys on this key's
/// string form. Every provider's carries its label as a prefix, so the
/// read side can tell them apart. A bare id names no provider: it is
/// what every row anidb.app's resolves wrote holds, and the
/// allanime-era rows before them — nothing migrates, and nothing the
/// app walks claims them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShowKey {
    /// The provider whose slug this is; `None` for a bare id.
    pub provider: Option<ProviderId>,
    /// The slug as the provider spells it.
    pub slug: String,
}

impl ShowKey {
    /// A key for `slug` on `provider`.
    pub fn new(provider: ProviderId, slug: impl Into<String>) -> Self {
        Self {
            provider: Some(provider),
            slug: slug.into(),
        }
    }

    /// The key a stored id names. A prefix the app does not know is
    /// part of the slug: only a known label qualifies an id, and a
    /// bare id names no provider — anidb.app's slugs, and the
    /// allanime-era ids that predate slugs, which parse but carry no
    /// words.
    #[must_use]
    pub fn parse(id: &str) -> Self {
        for provider in ProviderId::known() {
            if let Some(slug) = id
                .strip_prefix(provider.label())
                .and_then(|r| r.strip_prefix(':'))
            {
                return Self::new(provider, slug);
            }
        }
        Self {
            provider: None,
            slug: id.to_string(),
        }
    }

    /// The Kitsu-searchable words the slug carries, when it is
    /// slug-shaped — both providers spell theirs `words-id`.
    #[must_use]
    pub fn search_term(&self) -> Option<String> {
        slug_search_term(&self.slug)
    }
}

impl std::fmt::Display for ShowKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.provider {
            None => f.write_str(&self.slug),
            Some(other) => write!(f, "{}:{}", other.label(), self.slug),
        }
    }
}
