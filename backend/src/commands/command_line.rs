//! The command line a hand-off's subtitle flags are fitted to.

/// The subtitle flags a hand-off puts on `program`'s command line
/// beside `fixed`: all of them.
#[must_use]
pub fn flags_within_budget(_program: &str, _fixed: &[String], flags: Vec<String>) -> Vec<String> {
    flags
}

#[cfg(test)]
#[path = "command_line_test.rs"]
mod tests;
