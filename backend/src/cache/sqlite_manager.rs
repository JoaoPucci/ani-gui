//! An [`r2d2::ManageConnection`] for [`rusqlite::Connection`]s.
//!
//! The cache's pool used `r2d2_sqlite`'s manager until rusqlite moved
//! past every release of it; this is that manager, kept here so the
//! pool follows rusqlite without waiting on another crate. It opens
//! what the old one opened: a file with rusqlite's default flags, or a
//! private in-memory database every connection of one manager shares.
//!
//! Adapted from r2d2_sqlite 0.25.0
//! (<https://github.com/ivanceras/r2d2-sqlite>), under the MIT License:
//!
//! > Copyright (c) 2015 Jovansonlee Cesar
//! >
//! > Permission is hereby granted, free of charge, to any person
//! > obtaining a copy of this software and associated documentation
//! > files (the "Software"), to deal in the Software without
//! > restriction, including without limitation the rights to use,
//! > copy, modify, merge, publish, distribute, sublicense, and/or sell
//! > copies of the Software, and to permit persons to whom the
//! > Software is furnished to do so, subject to the following
//! > conditions:
//! >
//! > The above copyright notice and this permission notice shall be
//! > included in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! > EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
//! > OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! > NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
//! > HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
//! > WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! > FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
//! > OTHER DEALINGS IN THE SOFTWARE.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags};

type InitFn = dyn Fn(&mut Connection) -> rusqlite::Result<()> + Send + Sync + 'static;

#[derive(Debug)]
enum Source {
    File(PathBuf),
    /// A shared-cache in-memory database, named so that only this
    /// manager's connections reach it.
    Memory(String),
}

/// Opens the connections of an r2d2 pool over one SQLite database.
pub struct SqliteConnectionManager {
    source: Source,
    flags: OpenFlags,
    init: Option<Box<InitFn>>,
    /// For a memory source, one connection held for the manager's
    /// lifetime: SQLite drops an in-memory database with its last
    /// connection, and the pool may close every one it hands out.
    keep_alive: Mutex<Option<Connection>>,
}

impl fmt::Debug for SqliteConnectionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteConnectionManager")
            .field("source", &self.source)
            .field("flags", &self.flags)
            .field("init", &self.init.as_ref().map(|_| "InitFn"))
            .finish_non_exhaustive()
    }
}

impl SqliteConnectionManager {
    fn new(source: Source) -> Self {
        Self {
            source,
            flags: OpenFlags::default(),
            init: None,
            keep_alive: Mutex::new(None),
        }
    }

    /// A manager over the database file at `path`, which the default
    /// flags create when it is missing.
    pub fn file<P: AsRef<Path>>(path: P) -> Self {
        Self::new(Source::File(path.as_ref().to_path_buf()))
    }

    /// A manager over a private in-memory database, shared by every
    /// connection the manager opens and dropped with the manager.
    #[must_use]
    pub fn memory() -> Self {
        Self::new(Source::Memory(uuid::Uuid::new_v4().to_string()))
    }

    /// The same manager, opening its connections with `flags` in place
    /// of rusqlite's defaults. A memory manager needs
    /// [`OpenFlags::SQLITE_OPEN_URI`] among them.
    #[must_use]
    pub fn with_flags(self, flags: OpenFlags) -> Self {
        Self { flags, ..self }
    }

    /// The same manager, running `init` on every connection it opens
    /// (to set a pragma, say). A connection whose `init` fails is not
    /// handed out.
    #[must_use]
    pub fn with_init<F>(self, init: F) -> Self
    where
        F: Fn(&mut Connection) -> rusqlite::Result<()> + Send + Sync + 'static,
    {
        Self {
            init: Some(Box::new(init)),
            ..self
        }
    }

    fn open(&self) -> rusqlite::Result<Connection> {
        match &self.source {
            Source::File(path) => Connection::open_with_flags(path, self.flags),
            Source::Memory(id) => {
                let uri = format!("file:{id}?mode=memory&cache=shared");
                let mut keep_alive = self
                    .keep_alive
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if keep_alive.is_none() {
                    *keep_alive = Some(Connection::open_with_flags(&uri, self.flags)?);
                }
                Connection::open_with_flags(&uri, self.flags)
            }
        }
    }
}

impl r2d2::ManageConnection for SqliteConnectionManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    fn connect(&self) -> rusqlite::Result<Connection> {
        let mut conn = self.open()?;
        if let Some(init) = &self.init {
            init(&mut conn)?;
        }
        Ok(conn)
    }

    fn is_valid(&self, conn: &mut Connection) -> rusqlite::Result<()> {
        conn.execute_batch("")
    }

    fn has_broken(&self, _conn: &mut Connection) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "sqlite_manager_test.rs"]
mod tests;
