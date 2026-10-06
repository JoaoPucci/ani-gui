//! Reading hianime's markup by attribute, shared by the search page's
//! reader ([`super::parse`]) and the AJAX listings' ([`super::ajax`]).

/// The value of the first `name="…"` attribute in `s`, `name` given
/// with its `="` — read by the whole attribute name: a match is taken
/// only where the character before it cannot be part of a name, so
/// `data-title="…"` ahead of `title="…"` is stepped over rather than
/// read as the title.
pub(super) fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(found) = s[from..].find(name) {
        let at = from + found;
        let bounded = s[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'));
        if bounded {
            return s[at + name.len()..].split('"').next();
        }
        from = at + 1;
    }
    None
}
