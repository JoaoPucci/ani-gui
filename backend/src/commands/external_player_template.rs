//! Building an external player's argument vector from a command
//! template — the custom form and the token substitution it runs on;
//! split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::*;

/// Shared argv assembly for the three known players — same shape,
/// different flag names. Order: title, referrer, subtitles, URL last.
pub(super) fn build_argv_with_template(
    args: &LaunchArgs,
    title_flag: &str,
    referrer_flag: &str,
    sub_flag: SubFlag,
) -> Vec<String> {
    let mut argv = Vec::with_capacity(3 + args.subtitle_urls.len());
    if let Some(t) = &args.title {
        argv.push(format!("{title_flag}{t}"));
    }
    if let Some(r) = &args.referer {
        argv.push(format!("{referrer_flag}{r}"));
    }
    match sub_flag {
        SubFlag::Each(flag) => argv.extend(args.subtitle_urls.iter().map(|u| format!("{flag}{u}"))),
        SubFlag::First(flag) => {
            argv.extend(args.subtitle_urls.first().map(|u| format!("{flag}{u}")))
        }
    }
    argv.push(args.stream_url.clone());
    argv
}

/// Build argv for the Custom kind by shlex-splitting the template
/// and substituting placeholders per token. A token containing a
/// missing/empty placeholder is dropped from argv entirely so the
/// user can write `--referrer={referer}` without it landing as
/// `--referrer=` when the stream needs no referer.
///
/// Empty/None template falls back to URL only.
pub(super) fn build_argv_custom(args: &LaunchArgs) -> Vec<String> {
    let template = match args.custom_args_template.as_deref() {
        Some(s) if !s.trim().is_empty() => s,
        _ => return vec![args.stream_url.clone()],
    };
    let tokens = match shlex::split(template) {
        Some(t) => t,
        // Bad quoting in the template — fall back to bare URL so the
        // user at least sees the stream open instead of silently
        // failing.
        None => return vec![args.stream_url.clone()],
    };
    let referer = args.referer.as_deref().unwrap_or("");
    let title = args.title.as_deref().unwrap_or("");
    let subtitle = args.subtitle_urls.first().map_or("", String::as_str);
    let url = args.stream_url.as_str();
    tokens
        .into_iter()
        .filter_map(|tok| substitute_token(&tok, url, referer, title, subtitle))
        .collect()
}

/// Returns `Some(rendered)` when every placeholder in `tok` had a
/// non-empty value, `None` if any placeholder was empty (drop rule).
/// `{url}` is always present — tokens containing only `{url}` always
/// render. Unknown `{...}` placeholders pass through verbatim.
fn substitute_token(
    tok: &str,
    url: &str,
    referer: &str,
    title: &str,
    subtitle: &str,
) -> Option<String> {
    let mut out = String::with_capacity(tok.len());
    let mut chars = tok.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        // Read placeholder name up to `}`.
        let mut name = String::new();
        let mut closed = false;
        for nc in chars.by_ref() {
            if nc == '}' {
                closed = true;
                break;
            }
            name.push(nc);
        }
        if !closed {
            // Unterminated `{...` — pass through literally.
            out.push('{');
            out.push_str(&name);
            continue;
        }
        let value = match name.as_str() {
            "url" => url,
            "referer" => referer,
            "subtitle" => subtitle,
            "title" => title,
            // Unknown placeholder — preserve verbatim.
            other => {
                out.push('{');
                out.push_str(other);
                out.push('}');
                continue;
            }
        };
        if value.is_empty() {
            // Drop the entire token: a flag with an empty value is
            // worse than no flag at all.
            return None;
        }
        out.push_str(value);
    }
    Some(out)
}
