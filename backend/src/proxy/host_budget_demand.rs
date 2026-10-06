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
//! The player's other requests — playlists, keys, init segments, mp4
//! ranges, a segment whose URI names no kind — buy no playback the
//! budget can read, but each is a token the player waits for and that
//! counts toward its turns. So they count toward its need too, at the
//! rate they arrive ([`Demand::note_other`]): a playlist that names a
//! fresh key for every segment doubles what the player asks for, and
//! a live playlist refreshes as often as it has segments.

use std::collections::{HashMap, VecDeque};
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

/// How far back the player's other requests are counted. A request
/// older than this no longer says anything about the pace it asks at.
const OTHERS_WINDOW: Duration = Duration::from_secs(60);

/// The shortest span the other requests' rate is taken over, so the
/// first few, arriving together as playback starts, do not claim a
/// pace of several a second; it overstates rather than understates.
const OTHERS_FLOOR: Duration = Duration::from_secs(10);

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
    /// When the player's other requests arrived, within
    /// [`OTHERS_WINDOW`].
    others: VecDeque<Instant>,
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

    /// The player made a request that is not a segment of a stream.
    /// Requests past the window are dropped here as well as when the
    /// need is read, which happens only while background traffic
    /// waits.
    pub(crate) fn note_other(&mut self, now: Instant) {
        self.forget_others(now);
        self.others.push_back(now);
    }

    fn forget_others(&mut self, now: Instant) {
        while self
            .others
            .front()
            .is_some_and(|&at| now.saturating_duration_since(at) > OTHERS_WINDOW)
        {
            self.others.pop_front();
        }
    }

    /// What the player needs, as requests a second: its streams'
    /// segments, and its other requests at the rate they arrived over
    /// the last [`OTHERS_WINDOW`]. Streams it no longer plays and
    /// requests older than the window are forgotten.
    pub(crate) fn per_second(&mut self, now: Instant) -> f64 {
        self.streams.retain(|_, r| r.playing(now));
        self.forget_others(now);
        let segments: f64 = self
            .streams
            .values()
            .map(|r| 1.0 / r.segment.as_secs_f64())
            .sum();
        let others = self.others.front().map_or(0.0, |&oldest| {
            let span = now.saturating_duration_since(oldest).max(OTHERS_FLOOR);
            #[allow(clippy::cast_precision_loss)]
            let count = self.others.len() as f64;
            count / span.as_secs_f64()
        });
        segments + others
    }
}
