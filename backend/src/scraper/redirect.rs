//! Which redirect a request held to its origin follows — split from
//! the transport so the rule is pure and its file stays small.

/// The next URL a held request goes to: the target of a redirect
/// (`status` 3xx, `target` named) when it stays on `from`'s origin
/// (scheme, host and port), and `None` otherwise — no redirect, or
/// one that leaves the origin, whose response is then the answer.
pub fn same_origin_hop(from: &str, status: u16, target: Option<&str>) -> Option<String> {
    if !(300..400).contains(&status) {
        return None;
    }
    let from = url::Url::parse(from).ok()?;
    let next = from.join(target?).ok()?;
    (next.origin() == from.origin()).then(|| next.to_string())
}

#[cfg(test)]
#[path = "redirect_test.rs"]
mod tests;
