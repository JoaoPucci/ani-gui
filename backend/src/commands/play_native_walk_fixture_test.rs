//! The pick walk over real hianime pools: each fixture under
//! `tests/fixtures/hianime/picker/` is one Kitsu entry — the
//! arguments the app sends for it — with every search, episode list
//! and entry-page year the walk read for it from the live site,
//! reduced to what the client's parsers return. The walk runs over
//! them exactly as it runs over the site, so a pick that changes
//! here is a pick that changes for that show.

use super::*;
use crate::error::Result;
use crate::scraper::provider::{BrowseHit, EpisodeRef, ProviderId, StreamSource};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Entry {
    canonical: String,
    alt_titles: Vec<String>,
    episode_count: Option<u32>,
    year: Option<u32>,
    subtype: Option<String>,
}

#[derive(serde::Deserialize)]
struct Fixture {
    kitsu: Entry,
    searches: BTreeMap<String, Vec<(String, String, Option<String>)>>,
    episodes: BTreeMap<String, Vec<(u64, u32, Option<String>)>>,
    years: BTreeMap<String, Option<u32>>,
}

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/hianime/picker")
}

/// The site as one fixture recorded it. A request the recording does
/// not hold fails the test loudly: the walk asked something the live
/// run never did, and the fixture cannot say what the site answers.
struct Recorded {
    id: String,
    doc: Fixture,
}

impl Recorded {
    fn load(id: &str) -> Self {
        let path = fixture_dir().join(format!("{id}.json"));
        let body = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {id}: {e}"));
        Self {
            id: id.to_string(),
            doc: serde_json::from_str(&body).unwrap_or_else(|e| panic!("parse {id}: {e}")),
        }
    }
}

#[async_trait::async_trait]
impl Provider for Recorded {
    fn id(&self) -> ProviderId {
        ProviderId::Hianime
    }
    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        let hits = self
            .doc
            .searches
            .get(query)
            .unwrap_or_else(|| panic!("fixture {} holds no search for {query:?}", self.id));
        Ok(hits
            .iter()
            .map(|(slug, title, kind)| BrowseHit {
                slug: slug.clone(),
                title: title.clone(),
                kind: kind.clone(),
            })
            .collect())
    }
    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        let rows = self
            .doc
            .episodes
            .get(slug)
            .unwrap_or_else(|| panic!("fixture {} holds no listing for {slug}", self.id));
        Ok(rows
            .iter()
            .map(|(id, number, number2)| EpisodeRef {
                id: *id,
                number: *number,
                number2: number2.clone(),
            })
            .collect())
    }
    async fn has_mode(&self, _: u64, _: &str) -> Result<bool> {
        unreachable!("the pick walk reads no modes")
    }
    async fn master_playlist_url(&self, _: u64, _: &str) -> Result<StreamSource> {
        unreachable!("the pick walk reads no streams")
    }
    async fn playlist(&self, _: &str, _: Option<&str>) -> Result<String> {
        unreachable!("the pick walk reads no playlists")
    }
    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        Ok(*self
            .doc
            .years
            .get(slug)
            .unwrap_or_else(|| panic!("fixture {} holds no entry page for {slug}", self.id)))
    }
    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        None
    }
}

/// What the walk picks for fixture `id`, run with the entry's own
/// arguments.
async fn walk(id: &str) -> std::result::Result<PickedShow, NativeError> {
    let site = Recorded::load(id);
    let e = &site.doc.kitsu;
    pick_native_walk(
        &site,
        &e.canonical,
        &e.alt_titles,
        e.episode_count,
        e.year,
        e.subtype.as_deref(),
    )
    .await
}

/// Each row: a Kitsu id and what the walk must make of it — the slug
/// and the number of regular episodes the pick lists, numbered from
/// 1, or `None` for a pool the walk must refuse.
type Expectation = (&'static str, Option<(&'static str, u32)>);

async fn assert_picks(rows: &[Expectation]) {
    let mut wrong = Vec::new();
    for (id, want) in rows {
        let got = match walk(id).await {
            Ok(p) => {
                let count = super::super::play_native_numbering::regular_episode_count(&p.episodes);
                let first = p.episodes.iter().map(|e| e.number).min();
                if first.is_some() && first != Some(1) {
                    wrong.push(format!("{id}: listing starts at {first:?}"));
                }
                Some((p.hit.slug, count))
            }
            Err(e) => {
                assert!(
                    e.clean_miss || want.is_some(),
                    "{id}: a refusal must be the clean verdict, got {:?}",
                    e.error
                );
                None
            }
        };
        let want = want.map(|(slug, n)| (slug.to_string(), n));
        if got != want {
            wrong.push(format!("{id}: picked {got:?}, want {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "wrong picks:\n{}", wrong.join("\n"));
}

/// Entries a real-data audit found picked right, which must stay so.
const AUDITED_RIGHT: &[Expectation] = &[
    ("7158", Some(("jojos-bizarre-adventure-2012-708", 26))), // JoJo (2012)
    (
        "8063",
        Some(("jojos-bizarre-adventure-stardust-crusaders-718", 24)),
    ),
    (
        "11459",
        Some(("jojos-bizarre-adventure-diamond-is-unbreakable-1388", 39)),
    ),
    ("7442", Some(("attack-on-titan-240", 25))),
    ("8671", Some(("attack-on-titan-season-2-865", 12))),
    ("41982", Some(("attack-on-titan-season-3-part-2-1895", 10))),
    ("42422", Some(("attack-on-titan-final-season-1372", 16))),
    (
        "44240",
        Some(("attack-on-titan-final-season-part-2-868", 12)),
    ),
    ("43078", Some(("bleach-thousand-year-blood-war-1234", 13))),
    (
        "46903",
        Some(("bleach-thousand-year-blood-war-the-separation-1009", 13)),
    ),
    (
        "48015",
        Some(("bleach-thousand-year-blood-war-the-conflict-3859", 14)),
    ),
    ("45398", Some(("spy-x-family-639", 12))),
    ("45619", Some(("anime-14-256", 13))), // Spy x Family, Part 2
    ("46873", Some(("anime-13-255", 12))), // Spy x Family Season 2
    ("11578", Some(("mob-psycho-100-926", 12))),
    ("41071", Some(("mob-psycho-100-ii-918", 13))),
    ("41370", Some(("demon-slayer-kimetsu-no-yaiba-843", 26))),
    (
        "44081",
        Some((
            "demon-slayer-kimetsu-no-yaiba-entertainment-district-arc-749",
            11,
        )),
    ),
    (
        "11209",
        Some(("rezero-starting-life-in-another-world-1387", 25)),
    ),
    (
        "42198",
        Some(("rezero-starting-life-in-another-world-season-2-858", 13)),
    ),
    (
        "43247",
        Some((
            "rezero-starting-life-in-another-world-season-2-part-2-859",
            12,
        )),
    ),
    (
        "42323",
        Some(("mushoku-tensei-jobless-reincarnation-452", 11)),
    ),
    (
        "43907",
        Some(("mushoku-tensei-jobless-reincarnation-part-2-453", 12)),
    ),
    (
        "45950",
        Some(("mushoku-tensei-jobless-reincarnation-season-2-449", 12)),
    ),
    ("41084", Some(("vinland-saga-901", 24))),
    ("44875", Some(("vinland-saga-season-2-902", 24))),
    ("41373", Some(("kaguya-sama-love-is-war-1-437", 12))),
    ("42632", Some(("kaguya-sama-love-is-war-416", 12))), // Love is War Season 2
    ("47659", Some(("oshi-no-ko-season-2-682", 13))),
    ("49264", Some(("oshi-no-ko-season-3-672", 11))),
    (
        "46561",
        Some((
            "mobile-suit-gundam-the-witch-from-mercury-season-2-5101",
            12,
        )),
    ),
    ("42765", Some(("jujutsu-kaisen-237", 24))),
    ("45857", Some(("jujutsu-kaisen-season-2-415", 23))),
    ("9965", Some(("overlord-464", 13))),
    ("13237", Some(("overlord-ii-856", 13))),
    ("6028", Some(("fatezero-1115", 13))),
    ("7658", Some(("fatezero-season-2-1117", 12))),
    ("3919", Some(("bakemonogatari-976", 15))),
    ("42080", Some(("dr-stone-1235", 24))),
    ("44289", Some(("dr-stone-new-world-1104", 11))),
    ("8271", Some(("tokyo-ghoul-890", 12))),
    ("13929", Some(("tokyo-ghoulre-892", 12))),
    ("12", Some(("one-piece-1", 1180))),
    ("46474", Some(("frieren-beyond-journeys-end-481", 28))),
    ("46231", Some(("solo-leveling-235", 12))),
    (
        "48671",
        Some(("solo-leveling-season-2-arise-from-the-shadow-84", 13)),
    ),
    ("818", Some(("gintama-1237", 201))),
    ("5971", Some(("gintama-season-2-1153", 51))), // Gintama'
    (
        "41079",
        Some(("gintama-silver-soul-arc-second-half-war-1157", 14)),
    ),
    ("11", Some(("naruto-1335", 220))),
    ("1555", Some(("naruto-shippuden-600", 500))),
    ("13051", Some(("boruto-naruto-next-generations-650", 293))),
    ("42938", Some(("haikyu-to-the-top-2nd-cour-934", 12))),
    (
        "43361",
        Some((
            "that-time-i-got-reincarnated-as-a-slime-season-2-part-2-492",
            12,
        )),
    ),
    ("43066", Some(("86-eighty-six-1260", 11))),
    ("44398", Some(("86-eighty-six-part-2-1421", 12))), // Eighty Six: 2nd Season
    ("42068", Some(("fire-force-1107", 24))),
    ("13689", Some(("golden-kamuy-2595", 12))),
    ("45556", Some(("golden-kamuy-season-4-2598", 13))),
    ("186", Some(("ranma-3119", 161))),
    ("47083", Some(("the-apothecary-diaries-438", 24))),
    ("44973", Some(("blue-lock-964", 24))),
    ("13593", Some(("the-rising-of-the-shield-hero-885", 25))),
    ("6589", Some(("sword-art-online-874", 25))),
    ("46300", Some(("kaiju-no-8-257", 12))),
    ("48269", Some(("dan-da-dan-86", 12))),
    ("46229", Some(("mashle-magic-and-muscles-591", 12))),
];

#[tokio::test]
async fn entries_the_audit_found_picked_right_stay_picked_right() {
    assert_picks(AUDITED_RIGHT).await;
}

/// The manifest pins every fixture byte-for-byte, in both directions,
/// as the anidb fixtures' does.
#[test]
fn fixture_manifest_matches_the_fixtures() {
    use sha2::Digest as _;
    let dir = fixture_dir();
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("MANIFEST.json")).expect("read manifest"),
    )
    .expect("parse manifest");
    let entries = manifest.as_object().expect("manifest is an object");
    for (name, entry) in entries {
        let want = entry["sha256"].as_str().expect("sha256 entry");
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let have = format!("{:x}", sha2::Sha256::digest(&bytes));
        assert_eq!(
            have, *want,
            "{name}: bytes do not match the manifest digest"
        );
    }
    for file in std::fs::read_dir(&dir).expect("list fixtures") {
        let file = file.expect("dir entry").file_name();
        let file = file.to_string_lossy();
        assert!(
            file == "MANIFEST.json" || entries.contains_key(file.as_ref()),
            "{file} is not in MANIFEST.json"
        );
    }
}

/// Stone Ocean is TV on Kitsu and ONA on hianime: every listing of it
/// was disproven by format, and all three parts were unplayable.
#[tokio::test]
async fn a_series_one_catalogue_calls_tv_and_the_other_ona_is_found() {
    assert_picks(&[
        (
            "44294",
            Some(("jojos-bizarre-adventure-stone-ocean-1460", 12)),
        ),
        (
            "46010",
            Some(("jojos-bizarre-adventure-stone-ocean-part-2-1464", 12)),
        ),
        (
            "46598",
            Some(("jojos-bizarre-adventure-stone-ocean-part-3-2834", 14)),
        ),
    ])
    .await;
}
