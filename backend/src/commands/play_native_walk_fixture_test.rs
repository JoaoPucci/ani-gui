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

/// The manifest pins every fixture byte-for-byte, in both directions.
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

/// The listing that completes a span shares the spanning listing's
/// stem — a stem drops every marker a title ends on — so stem
/// identity alone would take it for the entry's own. When the entry's
/// titles name its number it is admitted, and at the entry's length it
/// would stop the cut and win: the second half played for the first.
#[tokio::test]
async fn the_listing_that_completes_a_span_is_never_the_entry_s_own() {
    let rows = [
        ("show-s2-1", "Show 2nd Season", 24),
        ("show-s2p2-2", "Show 2nd Season Part 2", 12),
    ];
    let site = pool_of(
        &["Show 2nd Season"],
        Some(12),
        2020,
        "Show 2nd Season",
        &rows,
    );
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!(
        (picked.hit.slug.as_str(), picked.episodes.len()),
        ("show-s2-1", 12)
    );
}

/// The division that completes a span is read in Japanese as in
/// English: 第2期 after a bare title, and a part after a 第3期 the
/// spanning listing itself ends on.
#[tokio::test]
async fn a_japanese_division_completes_a_spanning_listing() {
    type Pool<'a> = [(&'a str, &'a str, u32); 2];
    let cases: [(&str, Pool<'_>); 2] = [
        (
            "ショー",
            [("show-1", "ショー", 24), ("show-2", "ショー 第2期", 12)],
        ),
        (
            "進撃の巨人 第3期",
            [
                ("show-1", "進撃の巨人 第3期", 22),
                ("show-2", "進撃の巨人 第3期 Part 2", 10),
            ],
        ),
    ];
    for (title, rows) in cases {
        let site = pool_of(&[title], Some(12), 2020, title, &rows);
        let picked = walk_over(site).await.expect("the cut listing");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            ("show-1", 12),
            "{title}"
        );
    }
}

/// An entry named for its season by a bare number ("Show 2", "ショー２")
/// beside a listing of the whole show that runs past it: the listing
/// named for that season is the entry's own and stops the cut, though
/// it is exactly the remainder of the broad listing too. Only a
/// division the entry does not name — a later part — makes it the next
/// entry.
#[tokio::test]
async fn a_listing_named_for_the_entry_s_own_season_stops_a_cut() {
    type Pool<'a> = [(&'a str, &'a str, u32); 2];
    let cases: [(&str, Pool<'_>); 2] = [
        (
            "Show 2",
            [("show-1", "Show", 24), ("show-s2-2", "Show 2nd Season", 12)],
        ),
        (
            "ショー２",
            [("show-1", "ショー", 24), ("show-s2-2", "ショー 第2期", 12)],
        ),
    ];
    for (title, rows) in cases {
        let site = pool_of(&[title], Some(12), 2020, title, &rows);
        let picked = walk_over(site).await.expect("the entry's own listing");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            ("show-s2-2", 12),
            "{title}"
        );
    }
}

/// A sequel numbered with a bare number is held to what the entry's
/// titles name, as one numbered by a marker is: "Show 2" is not
/// "Show", alone in a pool at the entry's count or listed first beside
/// it.
#[tokio::test]
async fn a_sequel_numbered_with_a_bare_number_is_not_the_first_season() {
    let rows = [("show-2-1", "Show 2", 12)];
    let site = pool_of(&["Show"], Some(12), 2020, "Show", &rows);
    match walk_over(site).await {
        Err(e) => assert!(!e.clean_miss, "a refusal by title persisted"),
        Ok(p) => panic!("picked {}", p.hit.slug),
    }
    let rows = [("show-2-1", "Show 2", 12), ("show-2", "Show", 12)];
    let site = pool_of(&["Show: Arc"], Some(12), 2020, "Show: Arc", &rows);
    let picked = walk_over(site).await.expect("the first season");
    assert_eq!(picked.hit.slug, "show-2");
}

/// A broad listing ending on a bare number ("Show 2", "ショー２") is
/// read past its stem without the number by a sibling that does not
/// carry it, the number one of the broad listing's own divisions: a
/// sibling naming the next season by a marker completes it, and one
/// naming the broad listing's own season by a marker is the entry's
/// own. Each pool is `(entry titles, rows, the listing that must
/// play, its length)`.
#[tokio::test]
async fn a_broad_listing_ending_on_a_bare_number_is_read_past_it() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str, u32)], &'a str, usize);
    let cases: [Case<'_>; 4] = [
        (
            &["Show 2"],
            &[
                ("w", "Show 2", 24),
                ("s1", "Show", 12),
                ("l", "Show Season 3", 12),
            ],
            "w",
            12,
        ),
        (
            &["Show 2"],
            &[("w", "Show 2", 24), ("l", "Show 2nd Season Part 2", 12)],
            "w",
            12,
        ),
        (
            &["ショー２"],
            &[
                ("w", "ショー２", 24),
                ("s1", "ショー", 12),
                ("l", "ショー 第3期", 12),
            ],
            "w",
            12,
        ),
        (
            &["Show 2"],
            &[
                ("w", "Show 2", 24),
                ("own", "Show 2nd Season", 12),
                ("l", "Show Season 3", 12),
            ],
            "own",
            12,
        ),
    ];
    for (titles, rows, want, len) in cases {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        let picked = walk_over(site).await.expect("picked");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            (want, len),
            "{titles:?} {rows:?}"
        );
    }
}

/// Japanese writes a division straight after the title as often as
/// after a space: "ショー第2期" completes a span beside "ショー" as
/// "ショー 第2期" does.
#[tokio::test]
async fn a_japanese_division_glued_to_the_title_completes_a_spanning_listing() {
    let rows = [("show-1", "ショー", 24), ("show-2", "ショー第2期", 12)];
    let site = pool_of(&["ショー"], Some(12), 2020, "ショー", &rows);
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!(
        (picked.hit.slug.as_str(), picked.episodes.len()),
        ("show-1", 12)
    );
}

/// What makes a listing beside a spanning one the entry's own is
/// read against what the entry's titles name past the spanning
/// listing's stem, by ordinal, whatever kind the catalogues give it.
/// Each pool is `(entry titles, rows, the listing that must play, its
/// length)`.
#[tokio::test]
async fn the_entry_s_own_listing_is_what_its_titles_name_past_the_spanning_stem() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str, u32)], &'a str, usize);
    let cases: [Case<'_>; 4] = [
        // A number inside the spanning listing's own stem names no
        // season of the entry: "Lucky 2 2nd Season" is the next one.
        (
            &["Lucky 2"],
            &[("w", "Lucky 2", 24), ("l", "Lucky 2 2nd Season", 12)],
            "w",
            12,
        ),
        // A number in the entry's titles does not stop a first
        // season's listing being its own.
        (
            &["Kaiju No. 8", "怪獣８号"],
            &[
                ("w", "Kaiju No. 8", 23),
                ("l", "Kaiju No. 8 Part 2", 11),
                ("own", "Kaiju No. 8 Season 1", 12),
            ],
            "own",
            12,
        ),
        // The catalogues disagree on the kind: an entry ending on
        // Part 2 owns a listing named for a 2nd Season.
        (
            &["Show Part 2"],
            &[
                ("w", "Show Final Arc", 24),
                ("l", "Show Final Arc Part 3", 12),
                ("own", "Show Final Arc 2nd Season", 12),
            ],
            "own",
            12,
        ),
        // The listing that completes a span is the next entry even
        // when the entry's own listing stops the cut, whatever order
        // the provider lists them in.
        (
            &["Show"],
            &[
                ("w", "Show", 24),
                ("l", "Show Second Half", 12),
                ("own", "Show First Half", 12),
            ],
            "own",
            12,
        ),
    ];
    for (titles, rows, want, len) in cases {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        let picked = walk_over(site).await.expect("picked");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            (want, len),
            "{titles:?}"
        );
    }
}

/// Each ordinal the entry's titles name past the spanning stem makes
/// one division the entry's own, read the way the spanning listing
/// reads its own: "Show 2" owns the season of "Show 2nd Season Part 1"
/// and not the part of "… Part 2" besides; it owns a "2nd Season"
/// beside a "Show Part 2" whose 2 is a part. "Showtime 2" carries
/// another name than "Show", so nothing beside "Show" is its own and
/// nothing plays for it.
#[tokio::test]
async fn each_ordinal_past_the_spanning_stem_owns_one_division() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str, u32)], &'a str, usize);
    let cases: [Case<'_>; 4] = [
        (
            &["Show 2"],
            &[
                ("w", "Show", 24),
                ("own", "Show 2nd Season Part 1", 12),
                ("l", "Show 2nd Season Part 2", 12),
            ],
            "own",
            12,
        ),
        (
            &["Show 2"],
            &[
                ("w", "Show", 24),
                ("l", "Show 2nd Season Part 2", 12),
                ("own", "Show 2nd Season Part 1", 12),
            ],
            "own",
            12,
        ),
        (
            &["Show 2"],
            &[("w", "Show", 24), ("own", "Show 2nd Season Part 1", 12)],
            "own",
            12,
        ),
        (
            &["Show 2"],
            &[
                ("w", "Show Part 2", 24),
                ("own", "Show 2nd Season", 12),
                ("l", "Show 3rd Season", 12),
            ],
            "own",
            12,
        ),
    ];
    for (titles, rows, want, len) in cases {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        let picked = walk_over(site).await.expect("picked");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            (want, len),
            "{titles:?} {rows:?}"
        );
    }
    let site = pool_of(
        &["Showtime 2"],
        Some(12),
        2020,
        "Showtime 2",
        &[("w", "Show", 24), ("l", "Show 2nd Season", 12)],
    );
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} for Showtime 2", p.hit.slug);
    }
}

/// A listing is the entry's own beside a spanning one when it reads,
/// past the spanning listing's stem, as one of the entry's titles
/// reads there — the same reader on both sides, a bare number a
/// season on both, a first division the whole. An exact title is
/// its own; and a title naming one ordinal twice owns the listing
/// naming it twice.
#[tokio::test]
async fn a_listing_is_the_entry_s_own_when_it_reads_past_the_stem_as_the_entry_does() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str, u32)], &'a str, usize);
    let cases: [Case<'_>; 2] = [
        (
            &["Show 2"],
            &[
                ("w", "Show", 24),
                ("a", "Show 2", 12),
                ("b", "Show 2nd Season Part 2", 12),
            ],
            "a",
            12,
        ),
        (
            &["Show II Part 2: Arc"],
            &[
                ("w", "Show", 24),
                ("a", "Show 2nd Season Part 1", 12),
                ("b", "Show 2nd Season Part 2", 12),
            ],
            "b",
            12,
        ),
    ];
    for (titles, rows, want, len) in cases {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        let picked = walk_over(site).await.expect("picked");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            (want, len),
            "{titles:?} {rows:?}"
        );
    }
}

/// The whole of each title is read, by one reader on every side (the
/// decision table in title-resolution.md): an ordinal inside the
/// spanning listing's stem, a division the spanning listing shares
/// with the entry, or a weaker alias cannot make a listing look like
/// the entry's own; a numeral before a marker with its own ordinal is
/// a division of its own; and the entry's exact title is its own
/// whatever sits between the stem and its division.
#[tokio::test]
async fn the_whole_title_decides_the_entry_s_own_listing() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str, u32)], &'a str, usize);
    let cases: [Case<'_>; 6] = [
        (
            &["Show 2nd Season"],
            &[
                ("w", "Show 2", 24),
                ("a", "Show 2 Part 2", 12),
                ("b", "Show 3", 12),
            ],
            "w",
            12,
        ),
        (
            &["Show 2nd Season Part 2"],
            &[
                ("w", "Show 2nd Season Part 2", 24),
                ("a", "Show 2", 12),
                ("b", "Show Season 3", 12),
            ],
            "w",
            12,
        ),
        (
            &["Show Part 2"],
            &[
                ("w", "Show Part 2", 24),
                ("a", "Show Part 1", 12),
                ("b", "Show Season 3", 12),
            ],
            "w",
            12,
        ),
        (
            &["Show 2", "Show Season 3"],
            &[
                ("w", "Show 2nd Season", 24),
                ("a", "Show Part 2", 12),
                ("b", "Show Season 1", 12),
            ],
            "a",
            12,
        ),
        (
            &["Show II Part 2: Arc"],
            &[
                ("w", "Show II", 24),
                ("a", "Show II Part 1", 12),
                ("b", "Show II Part 2", 12),
            ],
            "b",
            12,
        ),
        (
            &["Show: Final Arc Season 2"],
            &[
                ("w", "Show", 24),
                ("a", "Show: Final Arc Season 2", 12),
                ("b", "Show 2nd Season", 12),
            ],
            "a",
            12,
        ),
    ];
    for (titles, rows, want, len) in cases {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        let picked = walk_over(site).await.expect("picked");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            (want, len),
            "{titles:?} {rows:?}"
        );
    }
}

/// The sibling that completes a broad listing is read by the same
/// reader as every other title: "II Part 2" right after the stem is a
/// season 2 and a part 2, never a lone part 2 that the broad listing's
/// own part could set aside.
#[tokio::test]
async fn the_completing_sibling_is_read_by_the_one_reader() {
    let site = pool_of(
        &["Show 2nd Season"],
        Some(12),
        2020,
        "Show 2nd Season",
        &[
            ("w", "Show Second Cour", 24),
            ("a", "Show 3", 12),
            ("b", "Show II Part 2", 12),
        ],
    );
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!((picked.hit.slug.as_str(), picked.episodes.len()), ("w", 12));
    let mut site = pool_of(
        &["Show 2", "Show Season 3"],
        Some(12),
        2020,
        "Show 2",
        &[
            ("w", "Show Season 3 Part 2", 24),
            ("a", "Show", 12),
            ("b", "Show II Part 2", 12),
        ],
    );
    // The walk searches the alias next with the same answer.
    let pool = site.doc.searches["Show 2"].clone();
    site.doc.searches.insert("Show Season 3".to_string(), pool);
    if let Ok(p) = walk_over(site).await {
        panic!("the completing sibling was picked: {}", p.hit.slug);
    }
}

/// A title's stem drops every division it ends on, Japanese ones
/// stacked one after another included: "ショー 第2期 第2部" has the
/// stem "ショー", so "ショー 第2期" beside it is read past that stem
/// and is "ショー２"'s own.
#[tokio::test]
async fn stacked_japanese_divisions_all_leave_the_stem() {
    let site = pool_of(
        &["ショー２"],
        Some(12),
        2020,
        "ショー２",
        &[
            ("w", "ショー 第2期 第2部", 24),
            ("a", "ショー", 12),
            ("b", "ショー 第2期", 12),
        ],
    );
    let picked = walk_over(site).await.expect("picked");
    assert_eq!((picked.hit.slug.as_str(), picked.episodes.len()), ("b", 12));
}

/// A spinoff — a title whose stem goes on past the broad listing's
/// with a word that names no division ("Side Story") — is another
/// show of the franchise, numbered or not: it never stops a cut and is
/// never the entry's own.
#[tokio::test]
async fn a_numbered_spinoff_is_never_the_entry_s_own() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str, u32)], &'a str, usize);
    let cases: [Case<'_>; 2] = [
        (
            &["Show Season 3"],
            &[
                ("w", "Show Season 3", 22),
                ("l", "Show Season 3 Part 2", 10),
                ("s", "Show Side Story 3", 12),
            ],
            "w",
            12,
        ),
        (
            &["Show 2"],
            &[
                ("w", "Show", 24),
                ("s", "Show Side Story 2", 12),
                ("own", "Show 2nd Season", 12),
            ],
            "own",
            12,
        ),
    ];
    for (titles, rows, want, len) in cases {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        let picked = walk_over(site).await.expect("picked");
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            (want, len),
            "{titles:?}"
        );
    }
}

/// Beside a broad listing the entry is not the head of, a listing
/// that opens past its stem on a first division only is that broad
/// listing's first part — "Show Part 1" beside "Show" — and never the
/// entry's: for "Show 2" the pool holds no listing of it.
#[tokio::test]
async fn a_first_part_beside_a_broad_listing_the_entry_does_not_head_is_not_the_entry() {
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[
            ("w", "Show", 24),
            ("a", "Show Part 1", 12),
            ("b", "Show 3rd Season", 12),
        ],
    );
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} for a later season", p.hit.slug);
    }
}

/// Beside a broad listing the entry is the head of, the sibling that
/// completes it is the next entry even when it reads as the entry —
/// "Show Part 2" reads 2 as "Show 2" does, kind aside — so the broad
/// "Show Season 2" is cut rather than the first season playing. Beside
/// one the entry does not head, a sibling that reads as the entry is
/// its own: "Show 2" at ten episodes beside "Show" still plays.
#[tokio::test]
async fn the_sibling_completing_a_listing_the_entry_heads_is_the_next_entry() {
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[
            ("w", "Show Season 2", 22),
            ("l", "Show Part 2", 10),
            ("s1", "Show", 12),
        ],
    );
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!((picked.hit.slug.as_str(), picked.episodes.len()), ("w", 12));
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[("w", "Show", 22), ("s2", "Show 2", 10)],
    );
    let picked = walk_over(site).await.expect("the entry's own listing");
    assert_eq!(picked.hit.slug, "s2");
}

/// Reading alike is not enough: a broad listing is the entry's to cut
/// only when it carries the name one of the entry's titles carries,
/// and a listing is the entry's own only when it starts with that
/// name. "Other Show Season 2" reads 2 as "Show Season 2" does, but it
/// is another show; neither it nor its second half plays.
#[tokio::test]
async fn another_show_reading_alike_is_not_the_entry() {
    for (titles, rows) in [
        (
            &["Show Season 2"][..],
            &[
                ("w", "Other Show Season 2", 24),
                ("l", "Other Show Season 2 Part 2", 12),
            ][..],
        ),
        (
            &["Show 2"][..],
            &[("w", "Other Show", 24), ("o", "Other Show 2nd Season", 12)][..],
        ),
    ] {
        let site = pool_of(titles, Some(12), 2020, titles[0], rows);
        if let Ok(p) = walk_over(site).await {
            panic!("{titles:?}: picked {} ({})", p.hit.slug, p.episodes.len());
        }
    }
}

/// When several broad listings qualify, the one the entry heads is
/// read, wherever it sits in the provider's order: an earlier broad
/// listing of another show beside its own first half does not stop the
/// search.
#[tokio::test]
async fn the_broad_listing_the_entry_heads_is_read_after_another_show_s() {
    let site = pool_of(
        &["Show"],
        Some(12),
        2020,
        "Show",
        &[
            ("o", "Other", 24),
            ("o1", "Other Part 1", 12),
            ("w", "Show", 22),
            ("l", "Show Part 2", 10),
        ],
    );
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!((picked.hit.slug.as_str(), picked.episodes.len()), ("w", 12));
}

/// A stem that ends in a digit takes no further digit glued to it:
/// "Show 20 Part 2" is another number, not "Show 2" with more after it,
/// so it completes no "Show 2" and nothing is cut for "Show 2".
#[tokio::test]
async fn a_numbered_stem_takes_no_further_digit() {
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[("w", "Show 2", 24), ("x", "Show 20 Part 2", 12)],
    );
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} ({})", p.hit.slug, p.episodes.len());
    }
}

/// Where W's stem ends on a bare number, the number a sibling carries
/// in its place decides: a higher one is the next season, the same one
/// written apart is W's own, and one that only continues W's digits is
/// another number, glued Japanese included.
#[tokio::test]
async fn the_number_where_w_s_stands_decides_how_a_sibling_reads() {
    let site = pool_of(
        &["Lucky 2"],
        Some(12),
        2020,
        "Lucky 2",
        &[("w", "Lucky 2", 24), ("l", "Lucky 3", 12)],
    );
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!((picked.hit.slug.as_str(), picked.episodes.len()), ("w", 12));
    let site = pool_of(
        &["ショー２"],
        Some(12),
        2020,
        "ショー２",
        &[
            ("w", "ショー２", 24),
            ("s1", "ショー", 12),
            ("own", "ショー ２", 12),
        ],
    );
    let picked = walk_over(site).await.expect("the entry's own listing");
    assert_eq!(picked.hit.slug, "own");
    let site = pool_of(
        &["ショー２"],
        Some(12),
        2020,
        "ショー２",
        &[("w", "ショー２", 24), ("x", "ショー２０第2部", 12)],
    );
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} ({})", p.hit.slug, p.episodes.len());
    }
}

/// A title whose words before its number match one of the entry's
/// titles but whose number only continues the entry's is another show:
/// refused, so with no broad listing to cut, nothing plays.
#[tokio::test]
async fn a_title_continuing_the_entry_s_number_is_another_show() {
    type Case<'a> = (&'a str, [(&'a str, &'a str, u32); 2]);
    let cases: [Case<'_>; 3] = [
        ("Show 2", [("w", "Show 2", 24), ("x", "Show20", 12)]),
        (
            "Mob Psycho 100 II",
            [
                ("w", "Mob Psycho 100 II", 22),
                ("x", "Mob Psycho 1000 Part 2", 10),
            ],
        ),
        ("86 Part 2", [("w", "86 Part 1", 24), ("x", "861", 12)]),
    ];
    for (title, rows) in cases {
        let site = pool_of(&[title], Some(12), 2020, title, &rows);
        if let Ok(p) = walk_over(site).await {
            panic!("{title}: picked {} ({})", p.hit.slug, p.episodes.len());
        }
    }
}

/// A title's first number ends at its word: a separate word that
/// starts with a digit after it never lengthens it, so these sequels,
/// each the entry's own listing, are not taken for a title continuing
/// the entry's number.
#[tokio::test]
async fn a_title_s_first_number_ends_at_its_word() {
    for (entry, listing) in [
        ("Kaiju No. 8 Season 2", "Kaiju No. 8 2nd Season"),
        ("Kaiju No. 8 Season 2", "Kaiju No. 8: 2nd Season"),
        ("Mob Psycho 100 II", "Mob Psycho 100 2nd Season"),
        ("86 Part 2", "86 2nd Season"),
        ("Show 2", "Show 2 (2020)"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("own", listing, 12)]);
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{entry}: {listing} refused: {e:?}"));
        assert_eq!(picked.hit.slug, "own", "{entry}");
    }
}

/// A Japanese number written in kanji is read as the number it is,
/// compound ones included, wherever a Japanese number is expected:
/// after 第 and before a counter.
#[tokio::test]
async fn a_kanji_number_is_read_as_the_number_it_is() {
    for (entry, listing) in [
        ("ショー 第十期", "Show Season 10"),
        ("ショー 第十二部", "Show Part 12"),
        ("ショー 第二十一期", "Show Season 21"),
        ("怪獣八号", "Kaiju No. 8"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("own", listing, 12)]);
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{entry}: {listing} refused: {e:?}"));
        assert_eq!(picked.hit.slug, "own", "{entry}");
    }
    let site = pool_of(
        &["怪獣八号"],
        Some(12),
        2020,
        "怪獣八号",
        &[("x", "怪獣八十号", 12)],
    );
    if let Ok(p) = walk_over(site).await {
        panic!("怪獣八号: picked {}", p.hit.slug);
    }
}

/// 第〇期 names the zeroth division, as 第0期 does.
#[tokio::test]
async fn a_kanji_zero_names_the_zeroth_division() {
    let site = pool_of(
        &["ショー 第〇期"],
        Some(12),
        2020,
        "ショー 第〇期",
        &[("own", "Show Season 0", 12)],
    );
    let picked = walk_over(site).await.expect("the entry's own listing");
    assert_eq!(picked.hit.slug, "own");
}

/// Word ordinals run to twentieth, cardinals after a marker to twenty
/// and Roman numerals to IX, read by every rule the grammar feeds: what
/// an entry names, what a candidate ends on, and the season before.
#[tokio::test]
async fn word_ordinals_above_sixth_are_read() {
    for (entry, listing) in [
        ("Show Seventh Season", "Show 7th Season"),
        ("Show Season Seven", "Show 7th Season"),
        ("Show Twelfth Season", "Show Season 12"),
        ("Show VII", "Show Season 7"),
        ("Spy x Family", "Spy x Family"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("own", listing, 12)]);
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{entry}: {listing} refused: {e:?}"));
        assert_eq!(picked.hit.slug, "own", "{entry}");
    }
    for (entry, listing) in [
        ("Show", "Show Seventh Season"),
        ("Show Seventh Season", "Show"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("x", listing, 12)]);
        if let Ok(p) = walk_over(site).await {
            panic!("{entry}: picked {}", p.hit.slug);
        }
    }
}

/// A word ordinal ends a title's name only where divisions alone
/// follow it: "The Seventh Son" is not named "The", so it heads no
/// broad listing for "The Seventh Sense".
#[tokio::test]
async fn a_word_ordinal_inside_a_name_does_not_end_it() {
    for (entry, w, l) in [
        (
            "The Seventh Sense",
            "The Seventh Son",
            "The Seventh Son Season 2",
        ),
        (
            "The Second Sense",
            "The Second Son",
            "The Second Son Season 2",
        ),
    ] {
        let site = pool_of(
            &[entry],
            Some(12),
            2020,
            entry,
            &[("w", w, 24), ("l", l, 12)],
        );
        if let Ok(p) = walk_over(site).await {
            panic!("{entry}: picked {} ({})", p.hit.slug, p.episodes.len());
        }
    }
}

/// An earlier season never completes a broad listing: it names a
/// season before the one the broad listing carries, wherever the
/// provider lists it, so nothing is cut.
#[tokio::test]
async fn an_earlier_season_never_completes_a_broad_listing() {
    type Pool<'a> = [(&'a str, &'a str, u32); 2];
    let cases: [(&str, Pool<'_>); 3] = [
        (
            "Show Season 3",
            [("w", "Show Season 3", 24), ("e", "Show Season 2", 12)],
        ),
        (
            "Show Season 3",
            [("e", "Show Season 2", 12), ("w", "Show Season 3", 24)],
        ),
        ("Show 2", [("w", "Show 2", 24), ("e", "Show Season 1", 12)]),
    ];
    for (entry, rows) in cases {
        let site = pool_of(&[entry], Some(12), 2020, entry, &rows);
        if let Ok(p) = walk_over(site).await {
            panic!(
                "{entry} {rows:?}: picked {} ({})",
                p.hit.slug,
                p.episodes.len()
            );
        }
    }
}

/// An ordinal standing alone after the broad listing's own season, in
/// the sibling, divides that season: "Show Season 3 Second Half"
/// completes "Show Season 3" as its part 2, and "Show 2nd Season Second
/// Half" completes "Show 2nd Season" though its "Second" equals W's 2.
#[tokio::test]
async fn an_ordinal_after_w_s_own_season_divides_it() {
    for (entry, w, l) in [
        (
            "Show Season 3",
            "Show Season 3",
            "Show Season 3 Second Half",
        ),
        ("Show Season 3", "Show Season 3", "Show Season 3: 2nd Half"),
        (
            "Show Season 3",
            "Show 3rd Season",
            "Show 3rd Season Second Half",
        ),
        (
            "Show 2nd Season",
            "Show 2nd Season",
            "Show 2nd Season Second Half",
        ),
    ] {
        let site = pool_of(
            &[entry],
            Some(12),
            2020,
            entry,
            &[("w", w, 24), ("l", l, 12)],
        );
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{w} + {l}: {e:?}"));
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            ("w", 12),
            "{w} + {l}"
        );
    }
}

/// A numeral is judged by its own form before a marker, and only the
/// broad listing's own season is divided by an ordinal after it —
/// spaced or glued to a Japanese season.
#[tokio::test]
async fn a_numeral_by_its_form_and_only_w_s_season_divided() {
    for (entry, w, l) in [
        ("Show II", "Show II", "Show II Part Two"),
        ("Show Part 2", "Show Part 2", "Show Part 2 Second Half"),
        ("ショー第3期", "ショー第3期", "ショー第3期２"),
    ] {
        let site = pool_of(
            &[entry],
            Some(12),
            2020,
            entry,
            &[("w", w, 24), ("l", l, 12)],
        );
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{w} + {l}: {e:?}"));
        assert_eq!(
            (picked.hit.slug.as_str(), picked.episodes.len()),
            ("w", 12),
            "{w} + {l}"
        );
    }
}

/// Only an ordinal right after the broad listing's own season divides
/// it: after its part, "Second" is a season 2, before W's season 3, and
/// completes nothing — spaced or glued in Japanese.
#[tokio::test]
async fn only_an_ordinal_right_after_w_s_season_divides_it() {
    for (entry, w, l) in [
        (
            "Show Season 3 Part 1",
            "Show Season 3 Part 1",
            "Show Season 3 Part 1 Second Half",
        ),
        ("ショー第3期第1部", "ショー第3期第1部", "ショー第3期第1部２"),
    ] {
        let site = pool_of(
            &[entry],
            Some(12),
            2020,
            entry,
            &[("w", w, 24), ("l", l, 12)],
        );
        if let Ok(p) = walk_over(site).await {
            panic!("{w} + {l}: picked {} ({})", p.hit.slug, p.episodes.len());
        }
    }
}

/// An entry every title of which names a later division refuses a
/// candidate naming none — the entry before it — unless that candidate
/// is one of its titles; an entry with any title naming none takes it.
#[tokio::test]
async fn an_entry_naming_only_later_divisions_refuses_the_one_before() {
    for (titles, listing) in [
        (&["Show 2"][..], "Show"),
        (&["Show Part 2"][..], "Show Season 1"),
        (&["Show 2"][..], "Show First Half"),
        (&["Show 2"][..], "Show Season 1 First Half"),
    ] {
        let site = pool_of(titles, Some(12), 2020, titles[0], &[("x", listing, 12)]);
        if let Ok(p) = walk_over(site).await {
            panic!("{titles:?}: picked {}", p.hit.slug);
        }
    }
    let site = pool_of(
        &["Show 2", "Show (2020)"],
        Some(12),
        2020,
        "Show 2",
        &[("x", "Show", 12)],
    );
    assert_eq!(walk_over(site).await.expect("picked").hit.slug, "x");
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[("s1", "Show", 12), ("own", "Show 2", 12)],
    );
    assert_eq!(walk_over(site).await.expect("picked").hit.slug, "own");
}

/// Only a candidate carrying the entry's name is the entry before it:
/// "Eighty Six" is a candidate for "86", "Show Season 1" for "ショー 第十期".
#[tokio::test]
async fn the_entry_before_carries_the_entry_s_name() {
    for (entry, listing) in [("86", "Eighty Six"), ("ショー 第十期", "Show Season 1")] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("x", listing, 12)]);
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{entry}: {listing} refused: {e:?}"));
        assert_eq!(picked.hit.slug, "x");
    }
}

/// A candidate carrying a number one of the entry's titles carries, as
/// that title does, is never the entry before it: "Show20" for an entry
/// also known as "Show 20".
#[tokio::test]
async fn a_carried_number_is_not_the_entry_before() {
    let site = pool_of(
        &["Show 2", "Show 20"],
        Some(12),
        2020,
        "Show 2",
        &[("x", "Show20", 12)],
    );
    let picked = walk_over(site).await.expect("the entry's own number");
    assert_eq!(picked.hit.slug, "x");
}

/// A provider's unnumbered name for a sequel adds a word to the
/// entry's name, so it is not the entry before it and plays.
#[tokio::test]
async fn an_unnumbered_sequel_name_is_not_the_entry_before() {
    for (entry, listing) in [
        ("Gintama 2", "Gintama'"),
        (
            "Kaguya-sama wa Kokurasetai 2",
            "Kaguya-sama wa Kokurasetai?",
        ),
        ("Haikyuu!! 4th Season", "Haikyuu!! To the Top"),
        ("Steins;Gate 0", "Steins;Gate Zero"),
        ("Show 2", "Show Two"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("x", listing, 12)]);
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{entry}: {listing} refused: {e:?}"));
        assert_eq!(picked.hit.slug, "x", "{entry}");
    }
}

/// A pool rejected after the titles scored out another entry of the
/// same show — the season before the broad listing, or the entry
/// before this one, with a count or without — is the clean miss it
/// would be without it, as one after the sequel guard's drop is.
#[tokio::test]
async fn another_entry_of_the_same_show_leaves_a_clean_miss() {
    let site = pool_of(
        &["Show Season 3 Part 1"],
        Some(12),
        2020,
        "Show Season 3 Part 1",
        &[
            ("w", "Show Season 3 Part 1", 24),
            ("p", "Show Season 3 Part 1 2nd Half", 12),
        ],
    );
    match walk_over(site).await {
        Err(e) => assert!(e.clean_miss, "season before W: {e:?}"),
        Ok(p) => panic!("picked {}", p.hit.slug),
    }
    for count in [Some(12), None] {
        let site = pool_of(&["Show 2"], count, 2020, "Show 2", &[("x", "Show", 12)]);
        match walk_over(site).await {
            Err(e) => assert!(e.clean_miss, "entry before, {count:?}: {e:?}"),
            Ok(p) => panic!("{count:?}: picked {}", p.hit.slug),
        }
    }
}

/// A separator between the name and its first divisions is no mark of
/// its own, and the name is compared word by word: each of these is
/// the entry before the entry, never picked.
#[tokio::test]
async fn a_separator_does_not_hide_the_entry_before() {
    for (entry, listing) in [
        ("Show 2", "Show: Season 1"),
        ("Oshi no Ko Season 2", "Oshi no Ko: Season 1"),
        ("Show: Season 2", "Show Season 1"),
        ("Steins;Gate 0", "Steins Gate"),
        ("ショー第2期", "ショー：第1期"),
        ("[Oshi no Ko] 2nd Season", "Oshi no Ko"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("x", listing, 12)]);
        if let Ok(p) = walk_over(site).await {
            panic!("{entry}: picked {} ({listing})", p.hit.slug);
        }
    }
}

/// A sibling naming a season before W's is a P whatever part it names
/// besides — however the season is written, and after a part too — so
/// it never completes W and nothing is cut.
#[tokio::test]
async fn a_season_before_w_disqualifies_a_sibling_whatever_its_part() {
    for (entry, wide, sibling) in [
        ("Show Season 3", "Show Season 3", "Show Season 2 Part 2"),
        ("Show Season 3", "Show Season 3", "Show II Part 2"),
        (
            "Show Season 3",
            "Show Season 3",
            "Show Second Season Part 2",
        ),
        ("Show Season 3", "Show Season 3", "Show Part 2 Season 2"),
        ("ショー 第3期", "ショー第3期", "ショー第二期第2部"),
        ("ショー 第3期", "ショー第3期", "ショー第2期第2部"),
    ] {
        let site = pool_of(
            &[entry],
            Some(12),
            2020,
            entry,
            &[("w", wide, 24), ("s", sibling, 12)],
        );
        if let Ok(p) = walk_over(site).await {
            panic!("{sibling} beside {wide}: picked {}", p.hit.slug);
        }
    }
}

/// A bracketed release year or format tag is neither a word nor a mark
/// to the entry before: each of these is the entry before "Show 2". Any
/// other bracketed words are words, so "Show (Shin Series)" plays.
#[tokio::test]
async fn a_year_or_format_tag_does_not_hide_the_entry_before() {
    for listing in [
        "Show Season 1 (2020)",
        "Show (TV)",
        "Show [OVA]",
        "Show (2020) Season 1",
        "Show （ONA）",
        "Show （ＴＶ）",
        "Show Season 1 （２０２０）",
    ] {
        let site = pool_of(&["Show 2"], Some(12), 2020, "Show 2", &[("x", listing, 12)]);
        if let Ok(p) = walk_over(site).await {
            panic!("{listing}: picked {}", p.hit.slug);
        }
    }
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[("x", "Show (Shin Series)", 12)],
    );
    assert_eq!(walk_over(site).await.expect("picked").hit.slug, "x");
}

/// A candidate naming a season before the entry's own is the season
/// before the entry, never picked — alone or beside the broad listing
/// the sequel guard drops — and its rejected pool is a clean miss. An
/// entry naming no season keeps the cross-kind allowance.
#[tokio::test]
async fn a_season_before_the_entry_s_own_is_never_picked() {
    for (entry, rows) in [
        (
            "Show Season 3 Part 2",
            &[("s", "Show Season 2 Part 2", 12)][..],
        ),
        ("Show Season 3 Part 2", &[("s", "Show Season 2", 12)][..]),
        ("Show 3 Part 2", &[("s", "Show 2 Part 2", 12)][..]),
        (
            "Show Season 3 Part 2",
            &[
                ("w", "Show Season 3", 24),
                ("s", "Show Season 2 Part 2", 12),
            ][..],
        ),
        ("ショー 第3期 第2部", &[("s", "ショー第二期第2部", 12)][..]),
    ] {
        for count in [Some(12), None] {
            let site = pool_of(&[entry], count, 2020, entry, rows);
            match walk_over(site).await {
                Ok(p) => panic!("{entry} {count:?}: picked {}", p.hit.slug),
                Err(e) => assert!(e.clean_miss, "{entry} {count:?}: {e:?}"),
            }
        }
    }
    let site = pool_of(
        &["86 Part 2"],
        Some(12),
        2021,
        "86 Part 2",
        &[("x", "Eighty Six: 2nd Season", 12)],
    );
    assert_eq!(walk_over(site).await.expect("picked").hit.slug, "x");
}

/// Hits that are another entry of the same show are never charged to
/// the probe head's five: five spellings of the entry before "Show 2"
/// ahead of it do not crowd "Show 2" out.
#[tokio::test]
async fn another_entry_is_never_charged_to_the_probe_head() {
    let site = pool_of(
        &["Show 2"],
        Some(12),
        2020,
        "Show 2",
        &[
            ("a", "Show", 12),
            ("b", "Show Season 1", 12),
            ("c", "Show Part 1", 12),
            ("d", "Show First Season", 12),
            ("e", "Show First Half", 12),
            ("own", "Show 2", 12),
        ],
    );
    assert_eq!(walk_over(site).await.expect("picked").hit.slug, "own");
}

/// Another entry of the same show could never have won, so its probe
/// dying unheard blocks no winner: a dead "Show" ahead of a live "Show
/// 2nd Season" for "Show 2", or a dead season before the entry ahead
/// of its own listing.
#[tokio::test]
async fn another_entry_s_dead_probe_blocks_no_winner() {
    for (entry, dead, own) in [
        ("Show 2", "Show", "Show 2nd Season"),
        (
            "Show Season 3 Part 2",
            "Show Season 2 Part 2",
            "Show 3rd Season Part 2",
        ),
    ] {
        let mut site = pool_of(
            &[entry],
            Some(12),
            2020,
            entry,
            &[("dead", dead, 12), ("own", own, 12)],
        );
        site.dead = vec!["dead"];
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{entry}: {e:?}"));
        assert_eq!(picked.hit.slug, "own", "{entry}");
    }
}

/// A broad listing that is another entry of the same show is never
/// cut: the entry heads "Show Second Season" through its "Show 2nd
/// Season" alias, but the listing names a season before the entry's
/// "Season 3".
#[tokio::test]
async fn a_broad_listing_that_is_another_entry_is_never_cut() {
    let rows = |n: u32| {
        (1..=n)
            .map(|k| format!("[{k}, {k}, null]"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let hits = r#"[["w", "Show Second Season", "TV"], ["l", "Show Second Season Part 2", "TV"]]"#;
    let json = format!(
        r#"{{
 "kitsu": {{"canonical": "Show Season 3", "alt_titles": ["Show 2nd Season"],
            "episode_count": 12, "year": 2020, "subtype": "TV"}},
 "searches": {{"Show Season 3": {hits}, "Show 2nd Season": {hits}}},
 "episodes": {{"w": [{w}], "l": [{l}]}},
 "years": {{"w": 2020, "l": 2020}}
}}"#,
        w = rows(24),
        l = rows(12),
    );
    let site = Recorded::inline("inline", &json);
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} ({})", p.hit.slug, p.episodes.len());
    }
}

/// A catalogue tag is not a title's tail: "Show Season 2 (2024)" and
/// "Show Part 2 (TV)" name a season and a part the unnumbered "Show"
/// does not, and "Show Season 2 (TV)" is "Show Season 2"'s own.
#[tokio::test]
async fn a_catalogue_tag_is_not_a_title_s_tail() {
    for count in [Some(12), None] {
        for listing in ["Show Season 2 (2024)", "Show Part 2 (TV)"] {
            let site = pool_of(&["Show"], count, 2020, "Show", &[("x", listing, 12)]);
            if let Ok(p) = walk_over(site).await {
                panic!("{listing} {count:?}: picked {}", p.hit.slug);
            }
        }
        let site = pool_of(
            &["Show Season 2"],
            count,
            2020,
            "Show Season 2",
            &[("x", "Show Season 2 (TV)", 12)],
        );
        assert_eq!(walk_over(site).await.expect("picked").hit.slug, "x");
    }
}

/// A catalogue tag is kept where two titles are compared as written:
/// it is what tells a remake from its original. Without a count, or
/// on a tie, the exact title wins in either direction, and an entry's
/// own tagged title is not its untagged namesake.
#[tokio::test]
async fn a_catalogue_tag_tells_a_remake_from_its_original() {
    for (entry, count, rows, want) in [
        (
            "Hunter x Hunter",
            None,
            [
                ("new", "Hunter x Hunter (2011)", 148),
                ("old", "Hunter x Hunter", 62),
            ],
            "old",
        ),
        (
            "Hunter x Hunter (2011)",
            None,
            [
                ("old", "Hunter x Hunter", 62),
                ("new", "Hunter x Hunter (2011)", 148),
            ],
            "new",
        ),
        (
            "Gintama",
            None,
            [("new", "Gintama (2011)", 51), ("old", "Gintama", 201)],
            "old",
        ),
        (
            "Fruits Basket",
            Some(25),
            [
                ("new", "Fruits Basket (2019)", 25),
                ("old", "Fruits Basket", 25),
            ],
            "old",
        ),
        (
            "Fruits Basket (2019)",
            Some(25),
            [
                ("old", "Fruits Basket", 25),
                ("new", "Fruits Basket (2019)", 25),
            ],
            "new",
        ),
        (
            "Show (2011)",
            Some(12),
            [("other", "Show", 12), ("own", "Show (2011)", 12)],
            "own",
        ),
    ] {
        for year in [2020, 0] {
            let mut site = pool_of(&[entry], count, 2020, entry, &rows);
            if year == 0 {
                site.doc.kitsu.year = None;
            }
            let picked = walk_over(site)
                .await
                .unwrap_or_else(|e| panic!("{entry}: {e:?}"));
            assert_eq!(picked.hit.slug, want, "{entry} {count:?} year {year}");
        }
    }
}
