//! Plumbing around the vendored `ani-cli` script.
//!
//! Playback resolution is native — the backend no longer spawns the
//! script. These modules keep the script itself healthy for the CLI
//! users sharing the install:
//!
//! - `parser` — the SSE progress-line grammar the overlay renders.
//! - `process` — locate the script on PATH / the bundled fallback.
//! - `update` — the auto-updater (`-U`) plus the carried fork patches
//!   it reapplies.

pub mod bash;
pub mod botan_shim;
mod capability;
mod carried_patches;
pub mod env;
pub mod parser;
pub mod process;
pub mod update;

pub use parser::{DebugOutput, SearchResult};
