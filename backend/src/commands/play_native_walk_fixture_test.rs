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
    /// Listings whose probe dies in transport, as weather would.
    dead: Vec<&'static str>,
}

impl Recorded {
    /// A recording written out in the test itself.
    fn inline(id: &str, json: &str) -> Self {
        Self {
            id: id.to_string(),
            doc: serde_json::from_str(json).unwrap_or_else(|e| panic!("parse {id}: {e}")),
            dead: Vec::new(),
        }
    }

    fn load(id: &str) -> Self {
        let path = fixture_dir().join(format!("{id}.json"));
        let body = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {id}: {e}"));
        Self {
            id: id.to_string(),
            doc: serde_json::from_str(&body).unwrap_or_else(|e| panic!("parse {id}: {e}")),
            dead: Vec::new(),
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
        if self.dead.contains(&slug) {
            return Err(crate::error::AniError::Network);
        }
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
    walk_over(Recorded::load(id)).await
}

/// [`walk`] over a recording the test has altered.
async fn walk_over(site: Recorded) -> std::result::Result<PickedShow, NativeError> {
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

/// The search for the canonical title leaves the first season's own
/// listing out of the pool, and its sequel sits within the count
/// tolerance and a year of it: `[Oshi no Ko]` picked "My Star:
/// Season 2", The Witch from Mercury its Season 2. A sibling named for
/// a season the entry never names must not be picked, and the walk
/// goes on to the alias whose pool holds the first season.
#[tokio::test]
async fn a_sequel_named_for_another_season_is_never_picked_for_the_first() {
    assert_picks(&[
        ("46170", Some(("oshi-no-ko-675", 11))),
        (
            "45217",
            Some(("mobile-suit-gundam-the-witch-from-mercury-3596", 12)),
        ),
    ])
    .await;
}

/// Boruto's announced second part has no year and no count on Kitsu,
/// and hianime carries no listing of it: the search answers with the
/// 293-episode series before it, which the pick took as the provider's
/// first hit. The part before the requested entry is never the entry,
/// so the pool is refused.
#[tokio::test]
async fn an_announced_part_is_refused_rather_than_resolved_to_the_series_before_it() {
    assert_picks(&[("47181", None)]).await;
}

/// Slime's second season: hianime lists "Season 2" and "2nd Season
/// Part 2" at twelve episodes each, both from 2021, Part 2 first. The
/// entry names no second part, so provider order must not decide.
#[tokio::test]
async fn of_two_same_length_cours_the_one_whose_part_agrees_is_picked() {
    assert_picks(&[(
        "42196",
        Some(("that-time-i-got-reincarnated-as-a-slime-season-2-487", 12)),
    )])
    .await;
}

/// hianime merges two Kitsu entries into one listing and also lists
/// the second on its own; the first entry picked the second's listing
/// on count. The merged listing serves the first entry, cut to its
/// episodes, which it numbers from 1.
#[tokio::test]
async fn a_listing_spanning_two_entries_serves_the_first_its_own_episodes() {
    assert_picks(&[
        // "Attack on Titan Season 3" (22) beside "Season 3 Part 2" (10).
        ("13569", Some(("attack-on-titan-season-3-866", 12))),
        // "Silver Soul Arc" (26) beside "Second Half War" (14).
        ("14095", Some(("gintama-silver-soul-arc-1155", 12))),
        // "To the Top (Part 1+2)" (25) beside "2nd Season" (12).
        ("42059", Some(("haikyu-to-the-top-933", 13))),
    ])
    .await;
}

/// The part tier ranks a candidate above the year, so a candidate
/// whose part agrees and whose probe died unheard outranks a winner
/// whose part does not: Slime's "Season 2" dying must not hand the
/// pick to "2nd Season Part 2", which would then be cached as the
/// entry's show.
#[tokio::test]
async fn a_dead_candidate_whose_part_agrees_blocks_the_one_whose_part_does_not() {
    let mut site = Recorded::load("42196");
    site.dead = vec!["that-time-i-got-reincarnated-as-a-slime-season-2-487"];
    // A transient pool sends the walk to the next alias, which the
    // live run never needed and the recording does not hold; the
    // canonical title's pool is the one under test.
    site.doc.kitsu.alt_titles.clear();
    match walk_over(site).await {
        Err(e) => assert!(
            !e.clean_miss && matches!(e.error, crate::error::AniError::Network),
            "the pick must be transient, got {:?}",
            e.error
        ),
        Ok(p) => panic!("picked {} with the agreeing cour unheard", p.hit.slug),
    }
}

/// Refusing a candidate by its title is an inference, and an inference
/// can be wrong: an entry whose titles carry its season only in a form
/// the rules do not read would have its own listing refused. A pool
/// rejected because the titles refused a candidate the count accepted
/// is therefore not the clean miss the availability cache persists.
#[tokio::test]
async fn a_pool_the_titles_refused_a_fitting_candidate_from_is_not_a_clean_miss() {
    for (subtitle, count) in [("Arc", Some(12)), ("Arc", None)] {
        let json = format!(
            r#"{{
 "kitsu": {{"canonical": "Show: {subtitle}", "alt_titles": [], "episode_count": {count},
            "year": 2020, "subtype": "TV"}},
 "searches": {{"Show: {subtitle}": [["show-season-2-1", "Show Season 2", "TV"]]}},
 "episodes": {{"show-season-2-1": [{rows}]}},
 "years": {{"show-season-2-1": 2020}}
}}"#,
            count = count.map_or("null".to_string(), |n: u32| n.to_string()),
            rows = (1..=12)
                .map(|n| format!("[{n}, {n}, null]"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        match walk_over(Recorded::inline("inline", &json)).await {
            Err(e) => assert!(
                !e.clean_miss,
                "count {count:?}: a refusal by title persisted"
            ),
            Ok(p) => panic!("count {count:?}: picked {}", p.hit.slug),
        }
    }
}

/// A candidate the entry's titles refuse could never have won, so its
/// probe dying unheard blocks no winner, however early it ranked.
#[tokio::test]
async fn a_refused_candidate_s_dead_probe_blocks_no_winner() {
    let rows = (1..=12)
        .map(|n| format!("[{n}, {n}, null]"))
        .collect::<Vec<_>>()
        .join(", ");
    let json = format!(
        r#"{{
 "kitsu": {{"canonical": "Show", "alt_titles": [], "episode_count": 12,
            "year": 2020, "subtype": "TV"}},
 "searches": {{"Show": [["show-season-2-1", "Show Season 2", "TV"],
                       ["show-tv-2", "Show TV", "TV"]]}},
 "episodes": {{"show-tv-2": [{rows}]}},
 "years": {{"show-season-2-1": 2020, "show-tv-2": 2020}}
}}"#
    );
    let mut site = Recorded::inline("inline", &json);
    site.dead = vec!["show-season-2-1"];
    let picked = walk_over(site).await.expect("the admitted candidate wins");
    assert_eq!(picked.hit.slug, "show-tv-2");
}

/// A pool written out as `(slug, title, aired episodes)` rows, every
/// listing from `year`, for an entry known by `titles`.
fn pool_of(
    titles: &[&str],
    count: Option<u32>,
    year: u32,
    query: &str,
    rows: &[(&str, &str, u32)],
) -> Recorded {
    let listing = |n: u32| {
        (1..=n)
            .map(|k| format!("[{k}, {k}, null]"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let json = serde_json::json!({
        "kitsu": {
            "canonical": titles[0],
            "alt_titles": titles[1..],
            "episode_count": count,
            "year": year,
            "subtype": "TV",
        },
        "searches": { query: rows.iter().map(|(s, t, _)| [*s, *t, "TV"]).collect::<Vec<_>>() },
        "episodes": rows.iter().map(|(s, _, n)| ((*s).to_string(), serde_json::from_str::<serde_json::Value>(&format!("[{}]", listing(*n))).unwrap())).collect::<serde_json::Map<_, _>>(),
        "years": rows.iter().map(|(s, _, _)| ((*s).to_string(), serde_json::json!(year))).collect::<serde_json::Map<_, _>>(),
    });
    Recorded::inline("inline", &json.to_string())
}

const SLIME_LIKE: [&str; 2] = ["Show 2", "Show 2nd Season Part 1"];

const SAME_LENGTH_COURS: [(&str, &str, u32); 2] = [
    ("show-s2p2-1", "Show 2nd Season Part 2", 5),
    ("show-s2-2", "Show Season 2", 5),
];

/// Slime's second season as it would look while airing: "Season 2"
/// and "2nd Season Part 2" at the same aired length and year, Part 2
/// first, both short of Kitsu's count. The airing rescue must prefer
/// the cour whose part agrees with the entry's, as winner selection
/// does.
#[tokio::test]
async fn the_airing_rescue_prefers_the_cour_whose_part_agrees() {
    let site = pool_of(&SLIME_LIKE, Some(24), 2021, "Show 2", &SAME_LENGTH_COURS);
    let picked = walk_over(site).await.expect("a cour is rescued");
    assert_eq!(picked.hit.slug, "show-s2-2");
}

/// The same pool without a count: the countless pick must prefer the
/// agreeing cour too.
#[tokio::test]
async fn the_countless_pick_prefers_the_cour_whose_part_agrees() {
    let site = pool_of(&SLIME_LIKE, None, 2021, "Show 2", &SAME_LENGTH_COURS);
    let picked = walk_over(site).await.expect("a cour is picked");
    assert_eq!(picked.hit.slug, "show-s2-2");
}

/// Two chains that stitch to the entry's count equally well, the one
/// led by the cour whose part disagrees listed first: the stitched
/// pick must be led by the agreeing cour.
#[tokio::test]
async fn a_stitched_chain_is_led_by_the_cour_whose_part_agrees() {
    let parts = [
        ("show-s2p2-1", "Show 2nd Season Part 2", 6),
        ("show-s2p2s-2", "Show 2nd Season Part 2 2nd Stage", 6),
        ("show-s2-3", "Show Season 2", 6),
        ("show-s2s-4", "Show Season 2 2nd Stage", 6),
    ];
    let site = pool_of(&SLIME_LIKE, Some(12), 2021, "Show 2", &parts);
    let picked = walk_over(site).await.expect("a chain is stitched");
    assert_eq!(picked.hit.slug, "show-s2-3");
    assert_eq!(
        picked.episodes.len(),
        12,
        "both parts of the agreeing chain"
    );
}

/// A spanning listing twice the entry's length beside an exact
/// listing of the entry: "Show" (24) and "Show Part 1" (12) for a
/// 12-episode entry. The exact listing is the entry's own and wins;
/// nothing is cut.
#[tokio::test]
async fn an_exact_listing_wins_over_cutting_one_twice_its_length() {
    let rows = [("show-1", "Show", 24), ("show-p1-2", "Show Part 1", 12)];
    let site = pool_of(&["Show"], Some(12), 2020, "Show", &rows);
    let picked = walk_over(site).await.expect("the exact listing");
    assert_eq!(
        (picked.hit.slug.as_str(), picked.episodes.len()),
        ("show-p1-2", 12)
    );
}

/// The sibling that completes a spanning listing is the next entry —
/// a later part than this one. A same-stem listing whose title names
/// no later part is no proof the broad listing holds two entries, and
/// nothing is cut for it.
#[tokio::test]
async fn only_a_later_part_completes_a_spanning_listing() {
    let rows = [
        ("show-1", "Show", 22),
        ("show-c-2", "Show Special Collection", 10),
    ];
    let site = pool_of(&["Show"], Some(12), 2020, "Show", &rows);
    if let Ok(picked) = walk_over(site).await {
        assert!(
            !(picked.hit.slug == "show-1" && picked.episodes.len() == 12),
            "the broad listing was cut on a sibling that names no later part"
        );
    }
}

/// An exact-count hit that shares nothing with a spanning pair is no
/// evidence the pair's entry has a listing of its own: the cut stands
/// and wins, whatever order the provider lists them in.
#[tokio::test]
async fn an_unrelated_exact_count_hit_does_not_stop_a_spanning_cut() {
    let rows = [
        ("other-1", "Unrelated Thing", 12),
        ("show-s3-2", "Show Season 3", 22),
        ("show-s3p2-3", "Show Season 3 Part 2", 10),
    ];
    let site = pool_of(&["Show Season 3"], Some(12), 2020, "Show Season 3", &rows);
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!(
        (picked.hit.slug.as_str(), picked.episodes.len()),
        ("show-s3-2", 12)
    );
}

/// Five listings named for seasons the entry is not, ranked ahead of
/// its own: the bounded probe head must not be spent on them, with a
/// count or without one.
#[tokio::test]
async fn refused_siblings_do_not_crowd_the_entry_out_of_the_probe_head() {
    let rows = [
        ("show-s2-1", "Show Season 2", 12),
        ("show-s3-2", "Show Season 3", 12),
        ("show-s4-3", "Show Season 4", 12),
        ("show-s5-4", "Show Season 5", 12),
        ("show-s6-5", "Show Season 6", 12),
        ("show-6", "Show", 12),
    ];
    for count in [Some(12), None] {
        let site = pool_of(&["Show"], count, 2020, "Show", &rows);
        let picked = walk_over(site).await.expect("the entry's own listing");
        assert_eq!(picked.hit.slug, "show-6", "count {count:?}");
    }
}

/// Only a listing with the spanning listing's own stem is the entry's
/// dedicated one: a same-year spinoff of the entry's length whose
/// title merely starts with the franchise name does not stop the cut.
#[tokio::test]
async fn a_spinoff_of_the_entry_s_length_does_not_stop_a_spanning_cut() {
    let rows = [
        ("show-ss-1", "Show Side Story", 12),
        ("show-s3-2", "Show Season 3", 22),
        ("show-s3p2-3", "Show Season 3 Part 2", 10),
    ];
    let site = pool_of(&["Show Season 3"], Some(12), 2020, "Show Season 3", &rows);
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!(
        (picked.hit.slug.as_str(), picked.episodes.len()),
        ("show-s3-2", 12)
    );
}

/// What completes a spanning listing is the next part, named as one
/// right after the shared name — not a spinoff that happens to carry
/// a number, nor a recap of the spanning listing's own season.
#[tokio::test]
async fn neither_a_numbered_spinoff_nor_a_recap_completes_a_spanning_listing() {
    type Pool<'a> = [(&'a str, &'a str, u32); 2];
    let cases: [(&str, Pool<'_>); 2] = [
        (
            "Show",
            [
                ("show-1", "Show", 22),
                ("show-ss2-2", "Show Side Story 2", 10),
            ],
        ),
        (
            "Show Season 3",
            [
                ("show-1", "Show Season 3", 22),
                ("show-rc-2", "Show Season 3 Recap", 10),
            ],
        ),
    ];
    for (title, rows) in cases {
        let site = pool_of(&[title], Some(12), 2020, title, &rows);
        if let Ok(picked) = walk_over(site).await {
            assert!(
                !(picked.hit.slug == "show-1" && picked.episodes.len() == 12),
                "{title}: the broad listing was cut on {:?}",
                rows[1].1
            );
        }
    }
}

/// Without a count, the hits the entry's titles refuse are dropped
/// before the year filter reads the head. When the year filter then
/// empties it, the pool was rejected partly by the title inference —
/// a refused hit from the entry's own year may have been its listing
/// — so the walk moves on without persisting a clean miss.
#[tokio::test]
async fn a_countless_pool_the_titles_narrowed_before_the_year_filter_is_not_a_clean_miss() {
    let json = r#"{
 "kitsu": {"canonical": "Show", "alt_titles": [], "episode_count": null,
            "year": 2020, "subtype": "TV"},
 "searches": {"Show": [["show-season-2-1", "Show Season 2", "TV"],
                       ["show-2", "Show", "TV"]]},
 "episodes": {},
 "years": {"show-season-2-1": 2020, "show-2": 2015}
}"#;
    match walk_over(Recorded::inline("inline", json)).await {
        Err(e) => assert!(!e.clean_miss, "a refusal by title persisted: {e:?}"),
        Ok(p) => panic!("picked {}", p.hit.slug),
    }
}
