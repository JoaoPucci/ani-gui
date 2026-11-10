//! The command-line budget a hand-off's subtitle flags are fitted to.
//!
//! The hand-offs put one flag per subtitle track on the player's
//! command line, and Windows bounds a command line, the program
//! included, at 32,767 UTF-16 units — 8,191 characters when the
//! program is a batch file, which runs through `cmd.exe`. A listing
//! can carry dozens of tracks, so the tracks go on in the order the
//! user most likely wants them until the next one would not fit, and
//! the rest are left behind. The budget is the same on every
//! platform, so a hand-off carries the same tracks whichever one it
//! runs on. Through Syncplay the flags are fitted to Syncplay's line
//! and to the player's it starts with them, whichever is shorter — a
//! batch-file player's, through `cmd.exe`; the few flags of its own
//! Syncplay adds there the worst-case count leaves room for, real
//! track URLs needing no escaping.

/// The longest command line `CreateProcessW` accepts, in UTF-16
/// units, less the terminating null.
pub const COMMAND_LINE_LIMIT: usize = 32_766;

/// The longest command `cmd.exe` accepts, which bounds a batch file's
/// command line.
pub const BATCH_COMMAND_LINE_LIMIT: usize = 8_191;

/// Whether `program` is a batch file, which Rust's std runs through
/// `cmd.exe` and quotes by `cmd.exe`'s rules: by its extension once
/// Windows resolves the path, which strips trailing dots and spaces.
fn is_batch(program: &str) -> bool {
    let lower = program
        .trim()
        .trim_end_matches(['.', ' '])
        .to_ascii_lowercase();
    lower.ends_with(".bat") || lower.ends_with(".cmd")
}

/// The most a program and its arguments may take for `program`.
#[must_use]
pub fn command_line_budget(program: &str) -> usize {
    if is_batch(program) {
        BATCH_COMMAND_LINE_LIMIT
    } else {
        COMMAND_LINE_LIMIT
    }
}

/// What `cmd.exe` is handed ahead of a batch file's own line:
/// `cmd.exe /e:ON /v:OFF /d /c "`, with room to spare.
const BATCH_PREFIX: usize = 32;

/// The shortest path a batch file is counted at. Its line names the
/// script by the path Windows resolves, which the configured name —
/// `run.bat` on the PATH — may not show.
const BATCH_PATH_FLOOR: usize = 260;

/// The most room `arg` can take on `program`'s Windows command line:
/// each argument quoted and spaced, every character escaped — at most
/// doubled, a quote or a backslash before one; for a batch file, a `%`
/// written `%%cd:~,%`, eight units for one byte. UTF-8 bytes bound
/// UTF-16 units from above, so the count never falls short of the
/// platform's.
#[must_use]
pub fn worst_case_len(program: &str, arg: &str) -> usize {
    let per_byte = if is_batch(program) { 8 } else { 2 };
    per_byte * arg.len() + 3
}

/// The room the program itself takes on its line. A batch file's path
/// is quoted there, not escaped.
fn program_len(program: &str) -> usize {
    if is_batch(program) {
        BATCH_PREFIX + program.len().max(BATCH_PATH_FLOOR) + 3
    } else {
        worst_case_len(program, program)
    }
}

/// The leading run of `flags` that fits beside `program` and `fixed`
/// under [`command_line_budget`]. The flags arrive most wanted first,
/// so the first that does not fit ends the run: what is kept is
/// always the most wanted, never a later flag that happened to be
/// shorter.
#[must_use]
pub fn flags_within_budget(program: &str, fixed: &[String], flags: Vec<String>) -> Vec<String> {
    let budget = command_line_budget(program);
    let mut used = program_len(program)
        + fixed
            .iter()
            .map(|a| worst_case_len(program, a))
            .sum::<usize>();
    let mut kept = Vec::with_capacity(flags.len());
    for flag in flags {
        let next = used + worst_case_len(program, &flag);
        if next > budget {
            break;
        }
        used = next;
        kept.push(flag);
    }
    kept
}

#[cfg(test)]
#[path = "command_line_test.rs"]
mod tests;
