//! The transfer and its sidecars, together.

use crate::error::Result;
use crate::scraper::provider::StreamSource;
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "download_transfer_test.rs"]
mod tests;
