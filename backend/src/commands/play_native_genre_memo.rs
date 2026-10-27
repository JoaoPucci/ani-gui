//! The pick's memo — split from `play_native_genre` so each file stays
//! inside the complexity ratchet's per-file bar.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AniError, Result};
use crate::scraper::provider::{
    BrowseHit, EntryPage, EpisodeRef, Provider, ProviderId, ResolvedStream, StreamSource,
};

/// A provider that answers each listing and entry page it has served
/// once from memory: the gate's later picks over the same pool repeat
/// no request the first one made. A probe that answered a dead
/// candidate is kept as that answer ([`replayed`]); a block is not.
pub(crate) struct Memo<'a, P: ?Sized> {
    inner: &'a P,
    episodes: Mutex<HashMap<String, std::result::Result<Vec<EpisodeRef>, AniError>>>,
    pages: Mutex<HashMap<String, EntryPage>>,
}

impl<'a, P: ?Sized> Memo<'a, P> {
    pub(crate) fn new(inner: &'a P) -> Self {
        Self {
            inner,
            episodes: Mutex::new(HashMap::new()),
            pages: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl<P: Provider + ?Sized> Provider for Memo<'_, P> {
    fn id(&self) -> ProviderId {
        self.inner.id()
    }
    fn label(&self) -> &'static str {
        self.inner.label()
    }
    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        self.inner.search(query).await
    }
    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        if let Some(kept) = self.episodes.lock().expect("memo").get(slug) {
            return match kept {
                Ok(rows) => Ok(rows.clone()),
                Err(e) => Err(replayed(e).expect("only a replayable failure is kept")),
            };
        }
        let answer = self.inner.episodes(slug).await;
        let keep = match &answer {
            Ok(rows) => Some(Ok(rows.clone())),
            Err(e) => replayed(e).map(Err),
        };
        if let Some(keep) = keep {
            self.episodes
                .lock()
                .expect("memo")
                .insert(slug.to_string(), keep);
        }
        answer
    }
    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        self.inner.has_mode(episode_id, mode).await
    }
    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        self.inner.master_playlist_url(episode_id, mode).await
    }
    async fn playlist(&self, url: &str, referer: Option<&str>) -> Result<String> {
        self.inner.playlist(url, referer).await
    }
    async fn playlist_at(&self, url: &str, referer: Option<&str>) -> Result<(String, String)> {
        self.inner.playlist_at(url, referer).await
    }
    async fn quality_stream_url(&self, source: &StreamSource, quality: &str) -> Result<String> {
        self.inner.quality_stream_url(source, quality).await
    }
    async fn stream_for(
        &self,
        episode_id: u64,
        mode: &str,
        quality: &str,
    ) -> Result<ResolvedStream> {
        self.inner.stream_for(episode_id, mode, quality).await
    }
    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        self.detail_page(slug).await.map(|page| page.year)
    }
    async fn detail_page(&self, slug: &str) -> Result<EntryPage> {
        if let Some(page) = self.pages.lock().expect("memo").get(slug) {
            return Ok(page.clone());
        }
        let page = self.inner.detail_page(slug).await?;
        self.pages
            .lock()
            .expect("memo")
            .insert(slug.to_string(), page.clone());
        Ok(page)
    }
    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        self.inner.last_attempt_at()
    }
    fn bound_attempt(&self, deadline: Option<tokio::time::Instant>) {
        self.inner.bound_attempt(deadline);
    }
}

/// The failure a probe answered, again, when the memo keeps it: the
/// answers the provider's listing probe gives for a dead candidate — a
/// listing that answered not-found-shaped, one that died in transport
/// or ran out of time, one whose body could not be read — are the same
/// dead candidate to every pick over the pool, so a pick made again
/// neither asks for it twice nor hears it answer where the first
/// weighed it dead. A block or a refused admit stops the pick; any
/// other failure, which the probe does not give, is not kept.
pub(super) fn replayed(e: &AniError) -> Option<AniError> {
    match e {
        AniError::Upstream { status } if !e.is_provider_block() => {
            Some(AniError::Upstream { status: *status })
        }
        AniError::Network => Some(AniError::Network),
        AniError::Timeout => Some(AniError::Timeout),
        AniError::ParseFailed { detail } => Some(AniError::ParseFailed {
            detail: detail.clone(),
        }),
        _ => None,
    }
}
