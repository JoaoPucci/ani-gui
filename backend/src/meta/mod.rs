//! Metadata client for Kitsu and AniList.
//!
//! Kitsu is the primary source for show metadata, search and episodes.
//! AniList supplies the Trending Now order, airing status and schedule,
//! the banner and per-episode thumbnail backfills where Kitsu has none,
//! the id bridging between the two, and the signed-in account's profile
//! and list. See `docs/architecture.md` §discovery.

pub mod anilist;
pub mod anilist_airing;
pub mod anilist_media;
pub mod anilist_streaming_eps;
pub mod anilist_user;
pub mod aniskip;
pub mod github;
pub mod images;
pub mod kitsu;
pub mod kitsu_page;
pub mod mal_user;
mod mal_user_net;
mod mal_user_parse;
