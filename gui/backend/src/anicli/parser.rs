//! Stdout parser for `ani-cli` invocations.
//!
//! Two output shapes the backend cares about:
//!
//! 1. Search results — emitted before the episode prompt:
//!    `<id>\t<title> (<n> episodes)`
//! 2. Debug-mode resolved stream — emitted by `ANI_CLI_PLAYER=debug`:
//!    ```text
//!    All links:
//!    <quality> >https://...
//!    <quality>cc>https://...
//!    subtitle >https://...
//!    m3u8_refr >https://...
//!    Selected link:
//!    https://...
//!    ```
//!
//! The functions here strip ANSI escapes, then run regex/split-based
//! extraction. They are pure (no I/O) and deterministic — ideal for
//! property tests.

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};

/// One row returned by `ani-cli`'s search step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Allanime show id (alphanumeric).
    pub id: String,
    /// Display title.
    pub title: String,
    /// Episode count for the active mode (sub or dub).
    pub episode_count: u32,
}

/// Parsed output of `ANI_CLI_PLAYER=debug ani-cli ...`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugOutput {
    /// The link `select_quality` chose.
    pub selected_url: String,
    /// All candidate links, in the order ani-cli emitted them.
    pub all_links: Vec<String>,
    /// `Referer:` value to send with stream requests, if any.
    pub referer: Option<String>,
    /// Subtitle .vtt URL, if any.
    pub subtitle_url: Option<String>,
}

/// Parse search-results lines into `SearchResult`s. The expected line
/// format is `id<TAB>title (N episodes)`, with a ` (YYYY)` release-year
/// tail appended since ani-cli 4.14.5. Lines that don't match the
/// pattern are silently skipped — `ani-cli` mixes log lines with results.
#[must_use]
pub fn parse_search_results(stdout: &str) -> Vec<SearchResult> {
    stdout.lines().filter_map(parse_search_line).collect()
}

/// Strip a trailing all-digit parenthetical — the release year ani-cli
/// ≥ 4.14.5 appends after the episode count. Returns the input slice
/// unchanged when the last group isn't purely digits (e.g. it's the
/// `(N episodes)` group on a pre-4.14.5 line).
fn strip_trailing_year(rest: &str) -> &str {
    let Some(inner) = rest.strip_suffix(')') else {
        return rest;
    };
    let Some(open) = inner.rfind('(') else {
        return rest;
    };
    let digits = &inner[open + 1..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return rest;
    }
    inner[..open].trim_end()
}

fn parse_search_line(line: &str) -> Option<SearchResult> {
    // Format: `id\ttitle (N episodes)` or `id\ttitle (N episodes) (YYYY)`
    let (id, rest) = line.split_once('\t')?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }

    // Peel off the year tail first so the rsplit below lands on the
    // episode-count group either way.
    let rest = strip_trailing_year(rest.trim_end());

    // The title may itself contain parentheses, so we rsplit on `(` to find
    // the last "(N episodes)" group.
    let (title_with_space, count_part) = rest.rsplit_once('(')?;
    let title = title_with_space.trim_end().to_string();
    if title.is_empty() {
        return None;
    }
    let count_part = count_part.trim();
    let count_str = count_part
        .strip_suffix(" episodes)")
        .or_else(|| count_part.strip_suffix(" episode)"))?;
    let episode_count = count_str.parse::<u32>().ok()?;
    Some(SearchResult {
        id: id.to_string(),
        title,
        episode_count,
    })
}

/// Classify a non-zero `ani-cli` exit from its (ANSI-stripped) stderr.
/// The script's `die()` messages are the only structured signal a
/// failed run leaves behind; both debug runners route their exit
/// handling through here so the mapping can't drift between the
/// buffered and streaming paths.
pub fn classify_failure_stderr(stderr: &str) -> AniError {
    if stderr.contains("No results found") {
        return AniError::NoResults;
    }
    if stderr.contains("Episode not released") {
        return AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_EPISODE_NOT_RELEASED,
        };
    }
    // dep_ch's die() prefix — shared by the plain "Please install
    // it." line and the Termux openssl hint. A local setup problem,
    // keyed separately so the scraper gate's recorders can ignore it.
    // 4.15's dep_ch dropped the quotes around the tool name while the
    // dep_ch_failover botan die kept them, so match the bare prefix;
    // both substrings must still co-occur.
    if stderr.contains("Program ") && stderr.contains("not found") {
        return AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_MISSING_DEP,
        };
    }
    AniError::Scraper {
        key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
    }
}

/// Parse `ANI_CLI_PLAYER=debug` output.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] if the stdout doesn't include the
/// `Selected link:` marker the debug branch is supposed to print.
pub fn parse_debug_output(stdout: &str) -> Result<DebugOutput> {
    let stdout = stdout.trim();

    // Find the "Selected link:" marker. Everything before it (after the
    // "All links:" header) is the link list; the line after the marker is
    // the chosen URL.
    let selected_idx = stdout
        .find("Selected link:")
        .ok_or_else(|| AniError::ParseFailed {
            detail: "no 'Selected link:' marker".into(),
        })?;

    let (links_part, after_selected) = stdout.split_at(selected_idx);
    let after_selected = after_selected
        .trim_start_matches("Selected link:")
        .trim_start();
    let selected_url = after_selected
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AniError::ParseFailed {
            detail: "no URL after 'Selected link:'".into(),
        })?
        .to_string();

    // Strip the optional "All links:" header and a trailing newline.
    let trimmed = links_part.trim();
    let links_block = trimmed
        .strip_prefix("All links:")
        .map_or(trimmed, str::trim_start);

    // Pull subtitle and m3u8_refr metadata lines out of the link list.
    let mut all_links = Vec::new();
    let mut subtitle_url = None;
    let mut referer = None;
    for raw in links_block.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("subtitle >") {
            subtitle_url = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("m3u8_refr >") {
            referer = Some(rest.trim().to_string());
            continue;
        }
        all_links.push(line.to_string());
    }

    Ok(DebugOutput {
        selected_url,
        all_links,
        referer,
        subtitle_url,
    })
}

#[cfg(test)]
#[path = "parser_test.rs"]
mod tests;
