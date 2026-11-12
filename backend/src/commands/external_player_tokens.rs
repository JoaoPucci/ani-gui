//! Which of a custom template's tokens reach the player's command
//! line, once their placeholders are filled; split from the template
//! module so each file stays inside the CRAP gate's per-file bar.

/// The template's `tokens` as `render` fills them, keeping only what
/// the template meant:
///
/// - a token `render` drops (a placeholder with no value) is left out;
/// - a token the template did not start as an option never becomes one
///   through what was substituted into it — the title is the show's
///   name, the rest the provider's — and is left out too;
/// - a value token left out (one the template did not write as an
///   option) takes with it the option the template wrote just before it
///   on its own (`--sub-file {subtitle}`), which alone would take the
///   template's next token, usually the stream, as its value. A dropped
///   option (`--referer={referer}`) leaves the option before it alone.
pub(super) fn kept_tokens(
    tokens: Vec<String>,
    render: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    // Whether the last token kept is an option the template wrote with
    // no value of its own.
    let mut bare_option_last = false;
    for tok in tokens {
        match render(&tok).filter(|r| !starts_option(r) || starts_option(&tok)) {
            Some(rendered) => {
                bare_option_last = rendered == tok && is_bare_option(&tok);
                out.push(rendered);
            }
            None => {
                if bare_option_last && !starts_option(&tok) {
                    out.pop();
                }
                bare_option_last = false;
            }
        }
    }
    out
}

/// Whether a command-line token reads as an option: `-…`, or `/…` as
/// many Windows players take theirs.
fn starts_option(token: &str) -> bool {
    token.starts_with('-') || token.starts_with('/')
}

/// An option written with no value joined to it, and no placeholder.
/// Not `--`, the end of options, not `-`, which names standard input,
/// not `/`, the root directory, and not a path (`/home/u/x.lua`): a
/// `/switch` has no second `/`.
fn is_bare_option(token: &str) -> bool {
    starts_option(token)
        && token != "--"
        && token != "-"
        && token != "/"
        && !token.contains('=')
        && !token.contains('{')
        && !token.get(1..).is_some_and(|rest| rest.contains('/'))
}
