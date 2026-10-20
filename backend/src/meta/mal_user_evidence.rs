//! MAL's answer to what the episode map asks of an anime; split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use serde::Deserialize;

use super::*;
use crate::account::episode_map::TrackerShow;

/// The fields the evidence read asks for beside the anime's id.
pub(super) const MAL_EVIDENCE_FIELDS: &str =
    "num_episodes,status,media_type,start_date,end_date,related_anime";

/// The anime as the episode map reads it: one bearer-scoped
/// `/anime/{id}` read with [`MAL_EVIDENCE_FIELDS`].
pub(super) async fn read(
    provider: &MalProvider,
    tokens: &Tokens,
    id: ProviderMediaId,
) -> Result<MediaEvidence> {
    let url = format!(
        "{}/anime/{}?fields={MAL_EVIDENCE_FIELDS}&nsfw=true",
        provider.api_url(),
        id.0
    );
    parse_media_evidence(&provider.get_auth_bytes(&url, tokens).await?)
}

/// Parse an `/anime/{id}?fields=num_episodes,status,media_type,start_date,
/// end_date,related_anime` response. MAL's `0` for a length not announced
/// is unknown; a date arrives as `YYYY-MM-DD`, `YYYY-MM` or `YYYY` and is
/// kept as it is; `related_anime` names the sequels but not their
/// lengths, which each one's own read answers.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't an anime.
pub(in crate::meta) fn parse_media_evidence(body: &[u8]) -> Result<MediaEvidence> {
    #[derive(Deserialize)]
    struct Wire {
        id: u32,
        #[serde(default)]
        num_episodes: Option<u32>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        media_type: Option<String>,
        #[serde(default)]
        start_date: Option<String>,
        #[serde(default)]
        end_date: Option<String>,
        #[serde(default)]
        related_anime: Vec<Related>,
    }
    #[derive(Deserialize)]
    struct Related {
        node: Node,
        #[serde(default)]
        relation_type: Option<String>,
    }
    #[derive(Deserialize)]
    struct Node {
        id: u32,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal anime evidence: {e}"),
    })?;
    Ok(MediaEvidence {
        show: TrackerShow {
            id: wire.id,
            id_mal: None,
            episodes: wire.num_episodes.filter(|n| *n > 0),
            finished: wire.status.as_deref() == Some("finished_airing"),
            start: wire.start_date,
            end: wire.end_date,
        },
        series: !matches!(wire.media_type.as_deref(), Some("movie" | "music")),
        sequels: wire
            .related_anime
            .into_iter()
            .filter(|r| r.relation_type.as_deref() == Some("sequel"))
            .map(|r| ProviderMediaId(r.node.id))
            .collect(),
        listed: None,
    })
}
