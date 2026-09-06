//! Native client for hianime — the second stream provider, and the
//! one `ani-cli` moved to when anidb.app went dark in September 2026.
//!
//! The flow: a search page (HTML) → per-entry episode list and
//! per-episode server list (JSON envelopes wrapping HTML, answered
//! only to `X-Requested-With`) → a server's base64 hash decodes to an
//! embed URL on a separate host → the embed page carries a blob that
//! XOR-decodes to the player's JSON, whose `src` is the master
//! playlist and whose `subtitles` are sidecar tracks. The CDN checks
//! the embed host's origin as `Referer` on every playlist fetch.

pub mod ajax;
pub mod embed;
pub mod parse;
pub use ajax::{parse_episode_list, parse_servers, preferred_server, ServerEmbed};
pub use embed::{decode_embed, embed_origin, EmbedPayload, SubtitleTrack};
pub use parse::{parse_detail_year, parse_search, slug_id};

#[cfg(test)]
#[path = "hianime_test.rs"]
mod tests;
