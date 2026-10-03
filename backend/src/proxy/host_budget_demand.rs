//! What the player needs from a host, and how much of a waiting line
//! that leaves background traffic.
//!
//! The player plays one stream of each kind at a time — one main
//! stream, one audio rendition, one subtitle rendition — and asks for a
//! segment of each as playback reaches it. Each media segment's proxied
//! URI names its kind and the playback it buys, so every fetch of one
//! tells the host's budget what the player needs; a level or track
//! switch replaces the stream of its kind. The host's budget leaves the
//! player that need, with [`PLAYER_HEADROOM`], before a waiting
//! background request gets a turn ([`player_turns`]).
//!
//! Init segments and keys carry no kind and are not counted. A level
//! switch fetches one init segment, and hls.js loads a key once per
//! key URI, so in the masters the player meets they are a handful of
//! requests against a segment per stream per few seconds.

use std::collections::HashMap;
use std::time::Duration;

use tokio::time::Instant;

/// A kind of stream the player plays one of at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stream {
    /// A variant: the main stream, video with or without its audio.
    Main,
    /// An `EXT-X-MEDIA` audio rendition.
    Audio,
    /// An `EXT-X-MEDIA` subtitle rendition.
    Subtitles,
}

impl Stream {
    /// How the kind is written into a proxied URI.
    #[must_use]
    pub fn slot(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Audio => "audio",
            Self::Subtitles => "subs",
        }
    }

    /// The kind a proxied URI names, if it names one.
    #[must_use]
    pub fn from_slot(slot: &str) -> Option<Self> {
        [Self::Main, Self::Audio, Self::Subtitles]
            .into_iter()
            .find(|s| s.slot() == slot)
    }
}

/// How much more than its streams need the player's share of a waiting
/// line has to be: the room a player behind its own playback has to
/// catch up in, and a buffer still filling has to grow in.
pub(crate) const PLAYER_HEADROOM: f64 = 1.25;

/// The shortest segment counted. A shorter one — a tiny trailing
/// segment, a broken duration — says nothing about the player's pace,
/// and counting it would claim a stream asking several times a second.
const SHORTEST_SEGMENT: Duration = Duration::from_millis(500);

/// The longest segment counted as itself; a longer one counts as this,
/// which overstates the need rather than understating it.
const LONGEST_SEGMENT: Duration = Duration::from_secs(60);

/// How many tokens the player waited for go before a waiting background
/// request gets one, given what the player's streams need, as requests
/// a second. One while alternating leaves the player its need with
/// [`PLAYER_HEADROOM`]; more while it needs more of the refill, enough
/// that its share covers the need with that headroom; `None` once the
/// need with its headroom is the whole refill or more, when background
/// traffic gets no turn while the player waits and takes only what the
/// player leaves — through a fill, which can last minutes, or for as
/// long as the player asks.
#[must_use]
pub(crate) fn player_turns(demand: f64, refill: Duration) -> Option<u32> {
    let rate = 1.0 / refill.as_secs_f64();
    let need = demand.max(0.0) * PLAYER_HEADROOM;
    if need <= rate / 2.0 {
        return Some(1);
    }
    if need >= rate {
        return None;
    }
    // k / (k + 1) of the refill covers the need once k reaches it.
    let turns = (need / (rate - need) - 1e-9).ceil();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((turns as u32).max(1))
}

/// A stream the player is playing: the playback each of its segments
/// buys, and when the player last fetched one.
#[derive(Debug, Clone, Copy)]
struct Rendition {
    segment: Duration,
    fetched_at: Instant,
}

impl Rendition {
    /// Whether the player still plays it: a segment fetched within four
    /// segments' worth of playback, or half a minute for short ones. A
    /// player that fell behind fetches each stream at least that often;
    /// one it left stops counting.
    fn playing(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.fetched_at)
            <= (self.segment * 4).max(Duration::from_secs(30))
    }
}

/// The streams the player plays from one host: at most one of each
/// kind.
#[derive(Debug, Default)]
pub(crate) struct Demand {
    streams: HashMap<Stream, Rendition>,
}

impl Demand {
    /// The player fetched a segment of `stream` buying `segment` of
    /// playback. A segment too short to say anything is not counted.
    pub(crate) fn note(&mut self, stream: Stream, segment: Duration, now: Instant) {
        if segment < SHORTEST_SEGMENT {
            return;
        }
        self.streams.insert(
            stream,
            Rendition {
                segment: segment.min(LONGEST_SEGMENT),
                fetched_at: now,
            },
        );
    }

    /// What the player's streams need, as requests a second; streams it
    /// no longer plays are forgotten.
    pub(crate) fn per_second(&mut self, now: Instant) -> f64 {
        self.streams.retain(|_, r| r.playing(now));
        self.streams
            .values()
            .map(|r| 1.0 / r.segment.as_secs_f64())
            .sum()
    }
}
