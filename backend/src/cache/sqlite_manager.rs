//! An [`r2d2::ManageConnection`] for [`rusqlite::Connection`]s.
//!
//! Not built yet: every method answers as a manager that cannot open
//! anything would.

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

type InitFn = dyn Fn(&mut Connection) -> rusqlite::Result<()> + Send + Sync + 'static;

/// Opens the connections of an r2d2 pool over one SQLite database.
pub struct SqliteConnectionManager {
    _init: Option<Box<InitFn>>,
}

impl fmt::Debug for SqliteConnectionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteConnectionManager").finish()
    }
}

impl SqliteConnectionManager {
    /// A manager over the database file at `path`.
    pub fn file<P: AsRef<Path>>(_path: P) -> Self {
        Self { _init: None }
    }

    /// A manager over a private in-memory database.
    #[must_use]
    pub fn memory() -> Self {
        Self { _init: None }
    }

    /// The same manager, opening its connections with `flags`.
    #[must_use]
    pub fn with_flags(self, _flags: OpenFlags) -> Self {
        self
    }

    /// The same manager, running `init` on every connection it opens.
    #[must_use]
    pub fn with_init<F>(self, _init: F) -> Self
    where
        F: Fn(&mut Connection) -> rusqlite::Result<()> + Send + Sync + 'static,
    {
        self
    }
}

impl r2d2::ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> rusqlite::Result<Connection> {
        Err(rusqlite::Error::InvalidQuery)
    }

    fn is_valid(&self, _conn: &mut Connection) -> rusqlite::Result<()> {
        Err(rusqlite::Error::InvalidQuery)
    }

    fn has_broken(&self, _conn: &mut Connection) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "sqlite_manager_test.rs"]
mod tests;
