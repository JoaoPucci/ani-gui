//! The loopback origins the backend listens on: one per class of
//! renderer traffic.
//!
//! Chromium opens at most six connections at a time to one host and
//! port over plain HTTP; a request past the sixth waits for a socket,
//! whatever it is for. The renderer's requests differ in how long they
//! hold one — an API call answered from the cache for milliseconds, an
//! availability probe or a play resolution for as long as a provider
//! takes, a download's progress stream for the whole download, a cold
//! poster for the upstream fetch — and on one origin the long holders
//! can fill all six and leave the player's segment fetches queued
//! behind them. A lane is a listener of its own, so each class spends a
//! connection budget of its own and none can starve another.
//!
//! Every lane serves the same router. Which lane a request reaches is
//! the renderer's choice (and the stream proxy's, which stamps its URLs
//! with the stream lane's origin); a URL composed against any lane
//! still answers, so nothing breaks when a caller picks the "wrong" one.

use std::net::SocketAddr;

use crate::error::Result;

/// A class of renderer traffic with a listener of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lane {
    /// Everything not named below: short request/response calls.
    Api,
    /// The stream proxy: master and rendition playlists, segments,
    /// keys, MP4 bodies and sidecar subtitles. What the player waits on.
    Stream,
    /// `/api/image`, which on a cold cache holds its socket for the
    /// upstream fetch.
    Image,
    /// Availability probes, which hold theirs until the provider walk
    /// answers or its deadline passes.
    Probe,
    /// Play resolutions' progress streams — the user's own and the
    /// prefetches that warm the next episodes.
    Resolve,
    /// Downloads' progress streams, each open for its whole download.
    Download,
}

impl Lane {
    /// Every lane, the API first.
    pub const ALL: [Self; 6] = [
        Self::Api,
        Self::Stream,
        Self::Image,
        Self::Probe,
        Self::Resolve,
        Self::Download,
    ];

    /// The name the handshake announces the lane under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Stream => "stream",
            Self::Image => "image",
            Self::Probe => "probe",
            Self::Resolve => "resolve",
            Self::Download => "download",
        }
    }
}

/// A lane's listener, bound.
pub struct BoundLane {
    /// Which lane it serves.
    pub lane: Lane,
    /// Where it is bound.
    pub addr: SocketAddr,
    /// The listener itself, handed to the server.
    pub listener: tokio::net::TcpListener,
}

/// Every lane's listener, in [`Lane::ALL`] order.
pub struct Lanes(Vec<BoundLane>);

impl Lanes {
    /// Bind a kernel-assigned loopback port for every lane.
    ///
    /// # Errors
    /// [`crate::AniError::Network`] when a bind fails.
    pub async fn bind() -> Result<Self> {
        let mut bound = Vec::with_capacity(Lane::ALL.len());
        for lane in Lane::ALL {
            let (addr, listener) = crate::proxy::bind_loopback(0).await?;
            bound.push(BoundLane {
                lane,
                addr,
                listener,
            });
        }
        Ok(Self(bound))
    }

    /// Where `lane` is bound.
    ///
    /// # Panics
    /// Never: [`Lanes::bind`] binds every lane.
    #[must_use]
    pub fn addr(&self, lane: Lane) -> SocketAddr {
        self.0
            .iter()
            .find(|b| b.lane == lane)
            .map(|b| b.addr)
            .expect("every lane is bound")
    }

    /// Every lane with its origin, `http://127.0.0.1:<port>`.
    #[must_use]
    pub fn bases(&self) -> Vec<(Lane, String)> {
        self.0.iter().map(|b| (b.lane, base(b.addr))).collect()
    }

    /// The listeners, for the server.
    #[must_use]
    pub fn into_listeners(self) -> Vec<tokio::net::TcpListener> {
        self.0.into_iter().map(|b| b.listener).collect()
    }
}

/// The origin a listener bound at `addr` is reached at.
#[must_use]
pub fn base(addr: SocketAddr) -> String {
    format!("http://{}:{}", addr.ip(), addr.port())
}

/// The handshake's lines, in the order they are printed:
///
/// ```text
/// ANI_GUI_LANE <name> <origin>      one per lane but the API's
/// ANI_GUI_LISTENING <api origin>
/// ANI_GUI_INTERNAL_SECRET <hex>
/// ```
///
/// The lanes come first because a reader may stop at the last two
/// lines, which are what the handshake has always ended with: the
/// Electron main process resolves once it has read both, so a lane
/// announced after them would be missed. A reader that knows nothing
/// of lanes reads the API's line as it always has, and still reaches
/// every route, since every lane serves them all.
#[must_use]
pub fn handshake_lines(bases: &[(Lane, String)], internal_secret: &str) -> Vec<String> {
    let mut lines: Vec<String> = bases
        .iter()
        .filter(|(lane, _)| *lane != Lane::Api)
        .map(|(lane, base)| format!("ANI_GUI_LANE {} {base}", lane.name()))
        .collect();
    if let Some((_, api)) = bases.iter().find(|(lane, _)| *lane == Lane::Api) {
        lines.push(format!("ANI_GUI_LISTENING {api}"));
    }
    lines.push(format!("ANI_GUI_INTERNAL_SECRET {internal_secret}"));
    lines
}

#[cfg(test)]
#[path = "lanes_test.rs"]
mod tests;
