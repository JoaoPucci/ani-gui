//! The checks behind a published notice's link, priority and release
//! range (see [`super::remote`]; its text fields are in
//! [`super::text_fields`]). Each returns the value to pass on or the
//! reason the notice is dropped.

use serde_json::Value;

use super::remote::{Priority, MAX_LINK_CHARS};

pub(super) fn link(raw: Option<&Value>) -> Result<Option<String>, &'static str> {
    let Some(raw) = raw else { return Ok(None) };
    let s = raw.as_str().ok_or("a link is not a string")?;
    let url = url::Url::parse(s).map_err(|_| "a link is not an address")?;
    let plain = matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|h| !h.is_empty());
    if !plain {
        return Err("a link is not a plain http or https address");
    }
    let out = url.as_str().to_string();
    if out.len() > MAX_LINK_CHARS {
        return Err("a link is past its limit");
    }
    Ok(Some(out))
}

pub(super) fn priority(raw: Option<&Value>) -> Result<Priority, &'static str> {
    match raw {
        None => Ok(Priority::Info),
        Some(v) => match v.as_str() {
            Some("info") => Ok(Priority::Info),
            Some("warning") => Ok(Priority::Warning),
            Some("critical") => Ok(Priority::Critical),
            _ => Err("a notice's priority is not one this build knows"),
        },
    }
}

/// Whether `app_version` falls within the optional `min`/`max` range.
pub(super) fn in_range(
    min: Option<&Value>,
    max: Option<&Value>,
    app_version: &str,
) -> Result<bool, &'static str> {
    let bound = |raw: Option<&Value>| -> Result<Option<(u64, u64, u64)>, &'static str> {
        raw.map(|v| {
            v.as_str()
                .and_then(triple)
                .ok_or("a version bound is not a version")
        })
        .transpose()
    };
    let (min, max) = (bound(min)?, bound(max)?);
    let Some(app) = triple(app_version) else {
        return Ok(true);
    };
    Ok(min.is_none_or(|m| app >= m) && max.is_none_or(|m| app <= m))
}

/// `X.Y.Z`, with any `-suffix` read as the release it precedes.
fn triple(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.split_once('-').map_or(v, |(c, _)| c);
    let mut parts = core.split('.');
    let mut next = || parts.next()?.parse::<u64>().ok();
    let t = (next()?, next()?, next()?);
    parts.next().is_none().then_some(t)
}
