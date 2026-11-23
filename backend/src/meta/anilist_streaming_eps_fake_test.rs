//! A stand-in for AniList that answers the streaming-listing and banner
//! queries — the single `Media(idMal:)` / `Media(id:)` lookups and the
//! batched `Page { media(idMal_in:) }` / `Page { media(id_in:) }` ones —
//! from a fixed set of media, the way AniList answers them: a page
//! holds at most fifty media and says when it was cut, a single lookup
//! by MAL id answers the first media recording it, and one it does not
//! index is the 404 `Media: null` answer. Shared by the batch's own
//! tests and the thumbnail and banner commands'.

use crate::meta::anilist_streaming_eps::{StreamingEntry, StreamingListing};

/// One media the stand-in knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FakeMedia {
    /// AniList's own id.
    pub(crate) id: u32,
    /// The MAL id AniList records for it, if any.
    pub(crate) mal: Option<u32>,
    /// How many numbered streaming entries it lists.
    pub(crate) listed: u32,
}

impl FakeMedia {
    /// The listing AniList's answer for this media reads as.
    pub(crate) fn listing(&self) -> StreamingListing {
        StreamingListing {
            episodes: Some(self.listed),
            finished: true,
            entries: (1..=self.listed)
                .map(|n| {
                    (
                        n,
                        StreamingEntry {
                            title: format!("T{}", self.id),
                            thumbnail: Some(format!("https://cr.cdn/{}/{n}.jpg", self.id)),
                        },
                    )
                })
                .collect(),
        }
    }

    /// The banner AniList records for this media: every media with an
    /// odd id has one.
    pub(crate) fn banner(&self) -> Option<String> {
        (self.id % 2 == 1).then(|| format!("https://al.cdn/banner/{}.jpg", self.id))
    }

    fn node(&self) -> serde_json::Value {
        let eps: Vec<serde_json::Value> = (1..=self.listed)
            .map(|n| {
                serde_json::json!({
                    "title": format!("Episode {n} - T{}", self.id),
                    "thumbnail": format!("https://cr.cdn/{}/{n}.jpg", self.id),
                })
            })
            .collect();
        serde_json::json!({
            "id": self.id,
            "idMal": self.mal,
            "episodes": self.listed,
            "status": "FINISHED",
            "streamingEpisodes": eps,
            "bannerImage": self.banner(),
        })
    }
}

/// The stand-in: the media it knows, in the order its pages list them.
#[derive(Debug, Clone, Default)]
pub(crate) struct FakeAniList {
    pub(crate) media: Vec<FakeMedia>,
}

/// What a request asked for, as the stand-in read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asked {
    /// `Page { media(idMal_in:) }` with these MAL ids.
    PageByMal(Vec<u32>),
    /// `Page { media(id_in:) }` with these AniList ids.
    PageById(Vec<u32>),
    /// `Media(idMal:)`.
    ByMal(u32),
    /// `Media(id:)`.
    ById(u32),
}

impl Asked {
    /// Read a request body the way the stand-in answers it; `None` for
    /// a body it does not know.
    pub(crate) fn of(body: &[u8]) -> Option<Self> {
        let body: serde_json::Value = serde_json::from_slice(body).ok()?;
        let vars = body.get("variables")?;
        let list = |v: &serde_json::Value| -> Option<Vec<u32>> {
            v.as_array()?
                .iter()
                .map(|x| x.as_u64().and_then(|x| u32::try_from(x).ok()))
                .collect()
        };
        let one = |v: &serde_json::Value| v.as_u64().and_then(|x| u32::try_from(x).ok());
        if let Some(v) = vars.get("malIds") {
            return list(v).map(Self::PageByMal);
        }
        if let Some(v) = vars.get("ids") {
            return list(v).map(Self::PageById);
        }
        if let Some(v) = vars.get("idMal") {
            return one(v).map(Self::ByMal);
        }
        vars.get("id").and_then(one).map(Self::ById)
    }
}

impl FakeAniList {
    /// The body and status AniList answers `asked` with.
    pub(crate) fn answer(&self, asked: &Asked) -> (u16, String) {
        let page = |hit: &dyn Fn(&FakeMedia) -> bool| {
            let found: Vec<serde_json::Value> = self
                .media
                .iter()
                .filter(|m| hit(m))
                .map(FakeMedia::node)
                .collect();
            let cut = found.len() > crate::meta::anilist_pages::PAGE_CAP;
            let shown: Vec<serde_json::Value> = found
                .into_iter()
                .take(crate::meta::anilist_pages::PAGE_CAP)
                .collect();
            serde_json::json!({
                "data": { "Page": { "pageInfo": { "hasNextPage": cut }, "media": shown } }
            })
            .to_string()
        };
        let single = |found: Option<&FakeMedia>| match found {
            Some(m) => (
                200,
                serde_json::json!({ "data": { "Media": m.node() } }).to_string(),
            ),
            None => (
                404,
                crate::meta::anilist_media::ANILIST_NOT_FOUND_BODY.to_string(),
            ),
        };
        match asked {
            Asked::PageByMal(ids) => (200, page(&|m| m.mal.is_some_and(|x| ids.contains(&x)))),
            Asked::PageById(ids) => (200, page(&|m| ids.contains(&m.id))),
            Asked::ByMal(mal) => single(self.media.iter().find(|m| m.mal == Some(*mal))),
            Asked::ById(id) => single(self.media.iter().find(|m| m.id == *id)),
        }
    }

    /// What the single-show path reads for a show Kitsu maps to `mal`
    /// and `anilist`: the media by MAL id first, by AniList id when
    /// AniList has none for the MAL id, and an empty listing when
    /// neither finds one.
    pub(crate) fn single_path(&self, mal: Option<u32>, anilist: Option<u32>) -> StreamingListing {
        let by_mal = mal.and_then(|x| self.media.iter().find(|m| m.mal == Some(x)));
        let by_id = || anilist.and_then(|x| self.media.iter().find(|m| m.id == x));
        by_mal
            .or_else(by_id)
            .map(FakeMedia::listing)
            .unwrap_or_default()
    }
}

impl FakeAniList {
    /// The banner the single-show path reads for a show Kitsu maps to
    /// `mal` and `anilist`, found the way [`Self::single_path`] finds
    /// its media.
    pub(crate) fn single_banner(&self, mal: Option<u32>, anilist: Option<u32>) -> Option<String> {
        let by_mal = mal.and_then(|x| self.media.iter().find(|m| m.mal == Some(x)));
        let by_id = || anilist.and_then(|x| self.media.iter().find(|m| m.id == x));
        by_mal.or_else(by_id).and_then(FakeMedia::banner)
    }
}

impl wiremock::Respond for FakeAniList {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        match Asked::of(&request.body) {
            Some(asked) => {
                let (status, body) = self.answer(&asked);
                wiremock::ResponseTemplate::new(status).set_body_string(body)
            }
            None => wiremock::ResponseTemplate::new(400),
        }
    }
}

/// What each request the server received asked for, in order.
pub(crate) async fn asked_of(server: &wiremock::MockServer) -> Vec<Asked> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter_map(|r| Asked::of(&r.body))
        .collect()
}

/// What each request for the stills (`streamingEpisodes`) the server
/// received asked for, in order.
pub(crate) async fn asked_for_stills(server: &wiremock::MockServer) -> Vec<Asked> {
    asked_where(server, "streamingEpisodes").await
}

/// What each request for banners the server received asked for.
pub(crate) async fn asked_for_banners(server: &wiremock::MockServer) -> Vec<Asked> {
    asked_where(server, "bannerImage").await
}

async fn asked_where(server: &wiremock::MockServer, field: &str) -> Vec<Asked> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| String::from_utf8_lossy(&r.body).contains(field))
        .filter_map(|r| Asked::of(&r.body))
        .collect()
}
