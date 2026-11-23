//! What the download lock logs, captured for a test to read.

/// Everything the capturing subscriber wrote.
#[derive(Clone, Default)]
pub(crate) struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Captured {
    /// The log so far, as text.
    pub(crate) fn text(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// A dispatcher that writes the lock's events, at every level, here.
    pub(crate) fn dispatch(&self) -> tracing::Dispatch {
        tracing::Dispatch::new(
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("ani_gui=debug"))
                .with_ansi(false)
                .with_writer(self.clone())
                .finish(),
        )
    }
}

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

/// Make callsites ask thread-local subscribers too. With no global
/// subscriber and a single live dispatcher, tracing can cache "never"
/// for a callsite first reached outside the capture (see
/// `meta::anilist_gate_test`); a second live dispatcher and a rebuilt
/// cache stop that. Hold the result for the test's length.
pub(crate) fn interest_from_every_thread() -> tracing::Dispatch {
    let second = tracing::Dispatch::new(tracing_subscriber::registry());
    tracing::callsite::rebuild_interest_cache();
    second
}
