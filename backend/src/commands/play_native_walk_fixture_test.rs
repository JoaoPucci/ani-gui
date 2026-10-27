//! The pick walk over real hianime pools: each fixture under
//! `tests/fixtures/hianime/picker/` is one Kitsu entry — the
//! arguments the app sends for it — with every search, episode list
//! and entry page the walk read for it from the live site, reduced to
//! what the client's parsers return: a search hit's slug, title,
//! badge and romanized name, an entry page's year, the other names
//! it gives the show and its genres. The walk runs over
//! them exactly as it runs over the site, so a pick that changes
//! here is a pick that changes for that show.

use super::*;
use crate::error::Result;
use crate::scraper::provider::{BrowseHit, EntryPage, EpisodeRef, ProviderId, StreamSource};
use crate::scraper::table_provider::LISTED_GENRE;
use std::collections::BTreeMap;

#[path = "play_native_walk_oracle_test.rs"]
mod oracle;

#[path = "play_native_walk_record_test.rs"]
mod record;

#[derive(serde::Deserialize)]
struct Entry {
    canonical: String,
    alt_titles: Vec<String>,
    episode_count: Option<u32>,
    year: Option<u32>,
    /// The premiere year AniList names for the entry where Kitsu knows
    /// no date, when the recording was made with both walks — the
    /// app's, and the one it would make with that year instead.
    #[serde(default)]
    anilist_year: Option<u32>,
    subtype: Option<String>,
}

#[derive(serde::Deserialize)]
struct Fixture {
    kitsu: Entry,
    searches: BTreeMap<String, Vec<(String, String, Option<String>)>>,
    episodes: BTreeMap<String, Vec<(u64, u32, Option<String>)>>,
    years: BTreeMap<String, Option<u32>>,
    /// The romanized name each search card carried beside its title,
    /// by slug; a card that carried none is not listed.
    #[serde(default)]
    jnames: BTreeMap<String, String>,
    /// The other names each entry page read gave the show — its
    /// Japanese title and its synonyms — by slug; a page that gave
    /// none is not listed.
    #[serde(default)]
    page_names: BTreeMap<String, Vec<String>>,
    /// The genres each entry page read listed, by slug; a page that
    /// listed none is not listed.
    #[serde(default)]
    genres: BTreeMap<String, Vec<String>>,
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
    /// A recording written out in a test rather than read from the
    /// site: a page it lists lists [`LISTED_GENRE`] unless the
    /// recording says what it lists, as the site's show pages do (319
    /// of the 321 the recordings hold list genres).
    synthetic: bool,
}

impl Recorded {
    /// A recording written out in the test itself.
    fn inline(id: &str, json: &str) -> Self {
        Self {
            id: id.to_string(),
            doc: serde_json::from_str(json).unwrap_or_else(|e| panic!("parse {id}: {e}")),
            dead: Vec::new(),
            synthetic: true,
        }
    }

    fn load(id: &str) -> Self {
        let path = fixture_dir().join(format!("{id}.json"));
        let body = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {id}: {e}"));
        Self {
            id: id.to_string(),
            doc: serde_json::from_str(&body).unwrap_or_else(|e| panic!("parse {id}: {e}")),
            dead: Vec::new(),
            synthetic: false,
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
                jname: self.doc.jnames.get(slug).cloned(),
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
                former_tag: None,
                title: None,
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
        match self.doc.years.get(slug) {
            Some(year) => Ok(*year),
            None => panic!("fixture {} holds no entry page for {slug}", self.id),
        }
    }
    async fn detail_page(&self, slug: &str) -> Result<EntryPage> {
        let listed = || {
            if self.synthetic {
                vec![LISTED_GENRE.to_string()]
            } else {
                Vec::new()
            }
        };
        Ok(EntryPage {
            year: self.detail_year(slug).await?,
            names: self.doc.page_names.get(slug).cloned().unwrap_or_default(),
            genres: self.doc.genres.get(slug).cloned().unwrap_or_else(listed),
            rating: None,
        })
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
        None,
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

/// The JoJo seasons hianime lists twice — an "(Uncensored)" copy and a
/// plain one, the same count and numbering — as `(Kitsu id, uncensored
/// copy, plain copy)`.
const JOJO_COPIES: &[(&str, &str, &str)] = &[
    (
        "7158",
        "jojos-bizarre-adventure-2012-708",
        "jojo-s-bizarre-adventure-9761",
    ),
    (
        "8063",
        "jojos-bizarre-adventure-stardust-crusaders-718",
        "jojo-s-bizarre-adventure-part-2-stardust-crusaders-9760",
    ),
    (
        "8739",
        "jojos-bizarre-adventure-stardust-crusaders-battle-in-egypt-710",
        "jojo-s-bizarre-adventure-part-3-stardust-crusaders-2nd-season-9759",
    ),
    (
        "11459",
        "jojos-bizarre-adventure-diamond-is-unbreakable-1388",
        "jojo-s-bizarre-adventure-part-4-diamond-is-unbreakable-9758",
    ),
    (
        "41410",
        "jojos-bizarre-adventure-golden-wind-695",
        "jojo-s-bizarre-adventure-part-5-golden-wind-9757",
    ),
    (
        "45512",
        "chained-soldier-uncensored-9454",
        "chained-soldier-3873",
    ),
    (
        "48639",
        "chained-soldier-season-2-uncensored-10415",
        "chained-soldier-season-2-5834",
    ),
];

/// Every JoJo season the provider lists twice is picked as one show
/// with two versions, the uncensored copy playing by default and the
/// plain one beside it — in the provider's order, as the walk heard
/// them.
#[tokio::test]
async fn every_jojo_season_listed_twice_offers_both_versions_uncensored_first() {
    use super::super::play_native_versions::VersionKind;
    let mut wrong = Vec::new();
    for (id, uncensored, plain) in JOJO_COPIES {
        let p = walk(id)
            .await
            .unwrap_or_else(|e| panic!("{id}: {:?}", e.error));
        if p.hit.slug != *uncensored {
            wrong.push(format!("{id}: plays {}, want {uncensored}", p.hit.slug));
        }
        let mut got: Vec<(VersionKind, &str)> = p
            .versions
            .iter()
            .map(|v| (v.kind, v.hit.slug.as_str()))
            .collect();
        got.sort_by_key(|(k, _)| *k == VersionKind::Standard);
        let want = vec![
            (VersionKind::Uncensored, *uncensored),
            (VersionKind::Standard, *plain),
        ];
        if got != want {
            wrong.push(format!("{id}: versions {got:?}, want {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Golden Wind's alias "Jojo's Bizarre Adventure Part 5: Golden Wind"
/// is the plain copy's exact title, which wins the pool's tie today;
/// asked by that alias the pick is still the season's two versions,
/// the uncensored one playing.
#[tokio::test]
async fn the_plain_copys_exact_title_still_plays_the_uncensored_version() {
    let site = Recorded::load("41410");
    let e = &site.doc.kitsu;
    let titles: Vec<&str> = std::iter::once(e.canonical.as_str())
        .chain(e.alt_titles.iter().map(String::as_str))
        .collect();
    let alias = "Jojo's Bizarre Adventure Part 5: Golden Wind";
    let hits = site.search(alias).await.expect("recorded");
    let p = pick_candidate_titled(
        &site,
        &hits,
        e.episode_count,
        alias,
        &titles,
        e.year,
        e.subtype.as_deref(),
    )
    .await
    .expect("picked");
    assert_eq!(p.hit.slug, "jojos-bizarre-adventure-golden-wind-695");
    assert_eq!(p.versions.len(), 2);
}

/// Entries a real-data audit found picked right — or, for a show the
/// catalogue does not list, rightly refused — which must stay so.
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
    (
        "8739",
        Some((
            "jojos-bizarre-adventure-stardust-crusaders-battle-in-egypt-710",
            24,
        )),
    ),
    (
        "41410",
        Some(("jojos-bizarre-adventure-golden-wind-695", 39)),
    ),
    // Chained Soldier: both seasons listed twice, the second season's
    // tagged copy "Chained Soldier Season 2 [Uncensored]".
    ("45512", Some(("chained-soldier-uncensored-9454", 12))),
    (
        "48639",
        Some(("chained-soldier-season-2-uncensored-10415", 12)),
    ),
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
    ("7442", Some(("attack-on-titan-240", 25))),
    ("8671", Some(("attack-on-titan-season-2-865", 12))),
    ("13569", Some(("attack-on-titan-season-3-866", 12))), // cut from 22, Part 2 behind
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
    ("46170", Some(("oshi-no-ko-675", 11))),              // [Oshi no Ko], listed as "My Star"
    ("47659", Some(("oshi-no-ko-season-2-682", 13))),
    ("49264", Some(("oshi-no-ko-season-3-672", 11))),
    (
        "45217",
        Some(("mobile-suit-gundam-the-witch-from-mercury-3596", 12)),
    ),
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
    ("14095", Some(("gintama-silver-soul-arc-1155", 12))), // cut from 26, Second Half War behind
    (
        "41079",
        Some(("gintama-silver-soul-arc-second-half-war-1157", 14)),
    ),
    ("11", Some(("naruto-1335", 220))),
    ("1555", Some(("naruto-shippuden-600", 500))),
    ("13051", Some(("boruto-naruto-next-generations-650", 293))),
    ("47181", None), // Boruto Part 2: not listed, refused
    ("42059", Some(("haikyu-to-the-top-933", 13))), // cut from 25 "(Part 1+2)"
    ("42938", Some(("haikyu-to-the-top-2nd-cour-934", 12))),
    (
        "42196",
        Some(("that-time-i-got-reincarnated-as-a-slime-season-2-487", 12)),
    ),
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

/// Dragon Ball Super and the entries named for it: every search for one
/// of them answers with the series, its two films, the Beerus remake
/// and the Super Dragon Ball Heroes shows. Each entry the catalogue
/// carries picks its own listing — the series by its count and year,
/// a film by its format, the remake airing short of its count by its
/// year.
#[tokio::test]
async fn entries_named_for_a_series_pick_their_own_listings() {
    assert_picks(&[
        ("10879", Some(("dragon-ball-super-1101", 131))),
        ("41046", Some(("dragon-ball-super-broly-2763", 1))),
        ("44544", Some(("dragon-ball-super-super-hero-1402", 1))),
        ("50497", Some(("dragon-ball-super-beerus-10542", 1))),
    ])
    .await;
}

/// Dragon Ball Z: The Real 4-D at Super Tenkaichi Budokai is a 2017
/// one-episode special hianime does not list: its searches answer with
/// the series, its films, and the Bardock and Trunks specials, which
/// premiered decades before it. The walk refuses every pool.
#[tokio::test]
async fn a_special_the_catalogue_does_not_list_is_refused() {
    assert_picks(&[("44661", None)]).await;
}

/// Dragon Ball Z: The Real 4-D is a 2016 one-episode special hianime
/// does not list. Its Japanese title's search answers with One Piece 4D,
/// a one-episode special from 2015 — a year the filter tolerates as a
/// December premiere, and a count of one that fits — and the pick
/// played it. Nothing but the count vouches for it, so the pool is
/// refused; the refusal overrides a fit on how the hit is named, so it
/// is not the clean miss the availability cache persists.
#[tokio::test]
async fn a_one_episode_special_from_another_show_is_not_the_entry() {
    match walk("13123").await {
        Ok(p) => panic!("picked {} ({})", p.hit.slug, p.hit.title),
        Err(e) => assert!(!e.clean_miss, "a refusal by name persisted: {:?}", e.error),
    }
}

/// The same pool with a one-episode crossover special in it — the
/// franchise's name among other shows' — whose entry page names no
/// year. A single video's count is one, so the count says nothing of
/// which video it is: without the entry's name or its year, the
/// special is not the entry, and the pool is refused rather than
/// played. Dropped as an inference, the refusal is no clean miss.
/// Where the entry page does name the special's year, the year rules
/// it out, as it does Bardock and Trunks.
#[tokio::test]
async fn a_single_video_with_neither_the_entry_s_name_nor_its_year_is_not_the_entry() {
    for (year, clean) in [(None, false), (Some(2013), true)] {
        let mut site = Recorded::load("44661");
        let slug = "dream-9-toriko-one-piece-dragon-ball-z-super-collaboration-special-1";
        site.doc
            .searches
            .get_mut("Dragon Ball Z: The Real 4-D at Super Tenkaichi Budokai")
            .expect("the canonical pool")
            .insert(
                0,
                (
                    slug.to_string(),
                    "Dream 9 Toriko & One Piece & Dragon Ball Z Super Collaboration Special"
                        .to_string(),
                    Some("SPECIAL".to_string()),
                ),
            );
        site.doc
            .episodes
            .insert(slug.to_string(), vec![(9, 1, None)]);
        site.doc.years.insert(slug.to_string(), year);
        // Its page lists a genre, as the site's show pages do.
        site.doc
            .genres
            .insert(slug.to_string(), vec![LISTED_GENRE.to_string()]);
        match walk_over(site).await {
            Ok(p) => panic!("year {year:?}: picked {}", p.hit.slug),
            Err(e) => assert_eq!(e.clean_miss, clean, "year {year:?}: {:?}", e.error),
        }
    }
}

/// A single video the entry names, or one whose year is the entry's,
/// is still the entry when its entry page says nothing more.
#[tokio::test]
async fn a_single_video_named_as_the_entry_or_from_its_year_is_picked() {
    let named = r#"{
 "kitsu": {"canonical": "Show: The Movie", "alt_titles": ["Show Gekijouban"],
            "episode_count": 1, "year": 2017, "subtype": "movie"},
 "searches": {"Show: The Movie": [["other-1", "Other Film", "MOVIE"],
                                  ["show-gekijouban-2", "Show Gekijouban", "MOVIE"]]},
 "episodes": {"other-1": [[1, 1, null]], "show-gekijouban-2": [[2, 1, null]]},
 "years": {"other-1": null, "show-gekijouban-2": null}
}"#;
    let picked = walk_over(Recorded::inline("inline", named))
        .await
        .expect("the film named as the entry");
    assert_eq!(picked.hit.slug, "show-gekijouban-2");

    let dated = r#"{
 "kitsu": {"canonical": "Show: The Movie", "alt_titles": [],
            "episode_count": 1, "year": 2017, "subtype": "movie"},
 "searches": {"Show: The Movie": [["other-1", "Other Film", "MOVIE"],
                                  ["show-film-2", "Show Film", "MOVIE"]]},
 "episodes": {"other-1": [[1, 1, null]], "show-film-2": [[2, 1, null]]},
 "years": {"other-1": null, "show-film-2": 2017}
}"#;
    let picked = walk_over(Recorded::inline("inline", dated))
        .await
        .expect("the film from the entry's year");
    assert_eq!(picked.hit.slug, "show-film-2");
}

/// Dragon Ball Super: Ginga Patrol is announced: Kitsu has no count
/// and no premiere date for it, and hianime does not list it yet. Every
/// search for it answers with Dragon Ball Super's pool, and the
/// countless pick took the series — its 131 episodes — as the
/// provider's first hit. With neither a count nor a year, nothing
/// vouches for a hit not named as the entry: the series, the remake
/// and the Heroes shows are other shows, and the pool is refused.
#[tokio::test]
async fn an_announced_entry_with_no_count_or_year_takes_no_hit_not_named_as_it() {
    assert_picks(&[("50496", None)]).await;
}

/// The same pool once the catalogue lists the entry, under the English
/// title Kitsu gives it rather than the one searched: the countless pick
/// takes it, behind the series and named only by an alias.
#[tokio::test]
async fn an_entry_with_no_count_or_year_takes_the_hit_named_as_it() {
    let mut site = Recorded::load("50496");
    let pool = site
        .doc
        .searches
        .get_mut("Dragon Ball Super: Ginga Patrol")
        .expect("the canonical pool");
    pool.insert(
        3,
        (
            "dragon-ball-super-the-galactic-patrol-1".to_string(),
            "Dragon Ball Super: The Galactic Patrol".to_string(),
            Some("TV".to_string()),
        ),
    );
    site.doc.episodes.insert(
        "dragon-ball-super-the-galactic-patrol-1".to_string(),
        vec![(1, 1, None)],
    );
    // Its page names no year and lists a genre, as the site's show
    // pages do: the pick reads the page of the listing it serves.
    site.doc
        .years
        .insert("dragon-ball-super-the-galactic-patrol-1".to_string(), None);
    site.doc.genres.insert(
        "dragon-ball-super-the-galactic-patrol-1".to_string(),
        vec![LISTED_GENRE.to_string()],
    );
    let picked = walk_over(site).await.expect("the entry's listing");
    assert_eq!(picked.hit.slug, "dragon-ball-super-the-galactic-patrol-1");
}

/// Named as the entry is its name and the divisions it reads as, not
/// the words they are written in: "Show Season 2" is "Show 2nd
/// Season", and an unrelated hit before it is not.
#[tokio::test]
async fn a_hit_reading_as_the_entry_is_named_as_it() {
    let json = r#"{
 "kitsu": {"canonical": "Show 2nd Season", "alt_titles": [], "episode_count": null,
            "year": null, "subtype": "TV"},
 "searches": {"Show 2nd Season": [["other-1", "Other Show", "TV"],
                                  ["show-season-2-2", "Show Season 2", "TV"]]},
 "episodes": {"other-1": [[1, 1, null]], "show-season-2-2": [[2, 1, null]]},
 "years": {"show-season-2-2": null}
}"#;
    let picked = walk_over(Recorded::inline("inline", json))
        .await
        .expect("the hit reading as the entry");
    assert_eq!(picked.hit.slug, "show-season-2-2");
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

/// For a dated entry with a count, a candidate no name vouches for
/// could never have won either, so its probe dying unheard blocks no
/// winner — though its page named the entry's year and it ranked ahead
/// of the winner, whose card carries the entry's name.
#[tokio::test]
async fn an_unnamed_candidate_s_dead_probe_blocks_no_winner() {
    let rows = (1..=12)
        .map(|n| format!("[{n}, {n}, null]"))
        .collect::<Vec<_>>()
        .join(", ");
    let json = format!(
        r#"{{
 "kitsu": {{"canonical": "Show", "alt_titles": [], "episode_count": 12,
            "year": 2020, "subtype": "TV"}},
 "searches": {{"Show": [["other-1", "Other", "TV"],
                       ["different-2", "Different", "TV"]]}},
 "episodes": {{"different-2": [{rows}]}},
 "years": {{"other-1": 2020, "different-2": 2020}},
 "jnames": {{"other-1": "Other", "different-2": "Show"}}
}}"#
    );
    let mut site = Recorded::inline("inline", &json);
    site.dead = vec!["other-1"];
    let picked = walk_over(site).await.expect("the named candidate wins");
    assert_eq!(picked.hit.slug, "different-2");
}

/// A pool written out as `(slug, title, aired episodes)` rows, every
/// listing from `year`, for an entry known by `titles`. Every card
/// carries the entry's canonical title as its romanized name, as the
/// provider's cards for an entry's own listings carry the entry's
/// romaji: the pools here are about how a listing's title reads, and
/// the name vouches for every hit, as the year does.
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
        "jnames": rows.iter().map(|(s, _, _)| ((*s).to_string(), serde_json::json!(titles[0]))).collect::<serde_json::Map<_, _>>(),
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

/// A broad listing that carries a subtitle the entry's titles do not is
/// another show, and the entry does not head it: "Show 2nd Season:
/// Extra Arc" (24) beside its "Part 2" (12) is not cut for a 12-episode
/// "Show 2nd Season" — its first twelve episodes are the extra arc's,
/// not the season's. Nothing else fits, so nothing plays.
#[tokio::test]
async fn a_broad_listing_with_a_subtitle_the_entry_lacks_is_never_cut_for_it() {
    let rows = [
        ("extra-1", "Show 2nd Season: Extra Arc", 24),
        ("extra-p2-2", "Show 2nd Season: Extra Arc Part 2", 12),
    ];
    let site = pool_of(
        &["Show 2nd Season"],
        Some(12),
        2020,
        "Show 2nd Season",
        &rows,
    );
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} ({} episodes)", p.hit.slug, p.episodes.len());
    }
}

/// A pool written out inline: an entry known by `titles` with `count`
/// and `year`, every title's search answering with the rows `(slug,
/// title, episodes, the year its page names)`.
fn inline_pool(
    titles: &[&str],
    count: Option<u32>,
    year: Option<u32>,
    subtype: &str,
    rows: &[(&str, &str, u32, Option<u32>)],
) -> Recorded {
    let listing = |n: u32| {
        serde_json::Value::Array((1..=n).map(|k| serde_json::json!([k, k, null])).collect())
    };
    let json = serde_json::json!({
        "kitsu": {
            "canonical": titles[0],
            "alt_titles": titles[1..],
            "episode_count": count,
            "year": year,
            "subtype": subtype,
        },
        "searches": titles.iter().map(|q| ((*q).to_string(), serde_json::json!(rows.iter().map(|(s, t, _, _)| [*s, *t, subtype]).collect::<Vec<_>>()))).collect::<serde_json::Map<_, _>>(),
        "episodes": rows.iter().map(|(s, _, n, _)| ((*s).to_string(), listing(*n))).collect::<serde_json::Map<_, _>>(),
        "years": rows.iter().map(|(s, _, _, y)| ((*s).to_string(), serde_json::json!(y))).collect::<serde_json::Map<_, _>>(),
    });
    Recorded::inline("inline", &json.to_string())
}

/// The marks a title is written with are part of what names it. A
/// prime tells Gintama' from the Gintama before it, and a release year
/// in brackets a film from its remake; neither is a word, and reading
/// titles by their words alone named the earlier show as the entry.
/// With no count and no year, a pool holding only "Gintama" is refused
/// for "Gintama'"; a one-video "Film (2020)" refuses "Film (2019)" whose
/// page names no year; the spanning cut does not head "Show" (24) for
/// "Show'" beside its Part 2 (12). Where the marks agree, each is still
/// taken.
#[tokio::test]
async fn a_title_s_marks_and_release_year_are_part_of_what_names_it() {
    let refused = [
        inline_pool(
            &["Gintama'"],
            None,
            None,
            "TV",
            &[("gintama-1", "Gintama", 201, None)],
        ),
        inline_pool(
            &["Gintama’"],
            None,
            None,
            "TV",
            &[("gintama-1", "Gintama", 201, None)],
        ),
        inline_pool(
            &["Film (2020)", "Film Gekijouban"],
            Some(1),
            Some(2020),
            "movie",
            &[("film-1", "Film (2019)", 1, None)],
        ),
        inline_pool(
            &["Show'"],
            Some(12),
            Some(2020),
            "TV",
            &[
                ("show-1", "Show", 24, Some(2020)),
                ("show-p2-2", "Show Part 2", 12, Some(2020)),
            ],
        ),
    ];
    let mut wrong = Vec::new();
    for site in refused {
        let entry = site.doc.kitsu.canonical.clone();
        if let Ok(p) = walk_over(site).await {
            wrong.push(format!("{entry}: picked {}", p.hit.slug));
        }
    }
    let taken = [
        (
            inline_pool(
                &["Gintama'"],
                None,
                None,
                "TV",
                &[
                    ("gintama-1", "Gintama", 201, None),
                    ("gintama-2-2", "Gintama′", 51, None),
                ],
            ),
            "gintama-2-2",
        ),
        (
            inline_pool(
                &["Film (2020)", "Film Gekijouban"],
                Some(1),
                Some(2020),
                "movie",
                &[
                    ("film-1", "Film (2019)", 1, None),
                    ("film-2", "Film Gekijouban", 1, None),
                ],
            ),
            "film-2",
        ),
    ];
    for (site, want) in taken {
        let entry = site.doc.kitsu.canonical.clone();
        match walk_over(site).await {
            Ok(p) if p.hit.slug == want => {}
            Ok(p) => wrong.push(format!("{entry}: picked {}, want {want}", p.hit.slug)),
            Err(e) => wrong.push(format!("{entry}: {:?}, want {want}", e.error)),
        }
    }
    assert!(wrong.is_empty(), "wrong picks:\n{}", wrong.join("\n"));
}

/// Only the marks that tell shows apart count: a prime, and a release
/// year where both titles carry one and they differ. Catalogues drift
/// on the rest — an exclamation or question mark, a decorative symbol,
/// a wave dash written one way or the other, a release year one side
/// leaves off, quotes single or double, an elision opening a word —
/// and a broad listing written without them is still the one the entry
/// heads, as an entry with no count or year still takes a hit written
/// without them.
#[tokio::test]
async fn marks_catalogues_drift_on_do_not_part_a_title_from_the_entry() {
    let spanning = |entry: &str, wide: &str| {
        let part2 = format!("{wide} Part 2");
        inline_pool(
            &[entry],
            Some(12),
            Some(2020),
            "TV",
            &[
                ("w-1", wide, 24, Some(2020)),
                ("p2-2", &part2, 12, Some(2020)),
            ],
        )
    };
    let cases = [
        (spanning("Remake (2020)", "Remake"), "w-1"),
        (spanning("Haikyuu!!", "Haikyuu"), "w-1"),
        (spanning("Lucky☆Star", "Lucky Star"), "w-1"),
        (spanning("Show 〜Arc〜", "Show ~Arc~"), "w-1"),
        (
            inline_pool(
                &["Toradora!"],
                None,
                None,
                "TV",
                &[
                    ("other-1", "Other Show", 12, None),
                    ("toradora-2", "Toradora", 25, None),
                ],
            ),
            "toradora-2",
        ),
        (
            inline_pool(
                &["Is It Wrong to Pick Up Girls in a Dungeon?"],
                None,
                None,
                "TV",
                &[(
                    "dungeon-1",
                    "Is It Wrong to Pick Up Girls in a Dungeon",
                    13,
                    None,
                )],
            ),
            "dungeon-1",
        ),
        (
            inline_pool(
                &["'Tis Time for \"Torture,\" Princess"],
                None,
                None,
                "TV",
                &[("tis-1", "Tis Time for Torture Princess", 12, None)],
            ),
            "tis-1",
        ),
        (
            inline_pool(
                &["Show 'Arc'"],
                None,
                None,
                "TV",
                &[("arc-1", "Show \"Arc\"", 12, None)],
            ),
            "arc-1",
        ),
    ];
    let mut wrong = Vec::new();
    for (site, want) in cases {
        let entry = site.doc.kitsu.canonical.clone();
        match walk_over(site).await {
            Ok(p) if p.hit.slug == want => {}
            Ok(p) => wrong.push(format!("{entry}: picked {}, want {want}", p.hit.slug)),
            Err(e) => wrong.push(format!("{entry}: {:?}, want {want}", e.error)),
        }
    }
    assert!(wrong.is_empty(), "wrong picks:\n{}", wrong.join("\n"));
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

/// A number glued to a word outside Japanese is the word's: "ＳＨＯＷ２"
/// is no season before "ＳＨＯＷ３", as "Show2" is none before "Show 3",
/// and is picked; glued to a Japanese word it is a number, refused by
/// title as "Show 2" is for "Show 3".
#[tokio::test]
async fn a_number_glued_to_a_word_outside_japanese_is_the_word_s() {
    for (entry, listing) in [
        ("ＳＨＯＷ３", "ＳＨＯＷ２"),
        ("Café3", "Café2"),
        ("Шоу3", "Шоу2"),
        ("Show 3", "Show2"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("x", listing, 12)]);
        match walk_over(site).await {
            Ok(p) => assert_eq!(p.hit.slug, "x", "{entry}"),
            Err(e) => panic!("{entry}: {listing} refused: {e:?}"),
        }
    }
    for (entry, listing) in [
        ("ショー３", "ショー２"),
        ("ｼｮｰ3", "ｼｮｰ2"),
        ("時々3", "時々2"),
    ] {
        let site = pool_of(&[entry], Some(12), 2020, entry, &[("x", listing, 12)]);
        match walk_over(site).await {
            Err(e) => assert!(e.refused_by_name && !e.clean_miss, "{entry}: {e:?}"),
            Ok(p) => panic!("{entry}: picked {}", p.hit.slug),
        }
    }
}

/// Digits glued before a letter outside Japanese are a word, not a
/// number in the stem's place: "Show 1é Season 1" opens past "Show"
/// on the word "1é", which names no division, so it names no season
/// before "Show 2" and is picked, as "Show Arc Season 1" would be read.
#[tokio::test]
async fn digits_before_a_letter_outside_japanese_are_no_number_in_the_stem_s_place() {
    for count in [Some(12), None] {
        let site = pool_of(
            &["Show 2"],
            count,
            2020,
            "Show 2",
            &[("x", "Show 1é Season 1", 12)],
        );
        match walk_over(site).await {
            Ok(p) => assert_eq!(p.hit.slug, "x", "{count:?}"),
            Err(e) => panic!("{count:?}: refused: {e:?}"),
        }
    }
}

/// An entry title that is nothing but a division has an empty stem,
/// which is no stem: nothing is read past it, so "1st Season" names no
/// season before "Season 2" any more than "Season 1" does, and both are
/// picked. "2nd Season" for "Season 3" names a 2 the entry does not, and
/// is refused by title as "Show 2nd Season" is for "Show Season 3".
#[tokio::test]
async fn nothing_is_read_past_an_empty_stem() {
    for count in [Some(12), None] {
        for (entry, listing) in [("Season 2", "Season 1"), ("Season 2", "1st Season")] {
            let site = pool_of(&[entry], count, 2020, entry, &[("x", listing, 12)]);
            match walk_over(site).await {
                Ok(p) => assert_eq!(p.hit.slug, "x", "{entry}, {listing}, {count:?}"),
                Err(e) => panic!("{entry}, {listing}, {count:?}: refused: {e:?}"),
            }
        }
        let site = pool_of(
            &["Season 3"],
            count,
            2020,
            "Season 3",
            &[("x", "2nd Season", 12)],
        );
        match walk_over(site).await {
            Err(e) => assert!(e.refused_by_name && !e.clean_miss, "{count:?}: {e:?}"),
            Ok(p) => panic!("{count:?}: picked 2nd Season ({})", p.hit.slug),
        }
    }
}

/// An empty name is no name for the entry before either, whatever marks
/// open the title: "・第1期" is not the entry before "・第2期", nor
/// "- Season 1" before "- Season 2"; each is picked.
#[tokio::test]
async fn an_empty_name_names_no_entry_before() {
    for (entry, listing) in [
        ("・第2期", "・第1期"),
        ("- Season 2", "- Season 1"),
        (": Season 2", ": Season 1"),
    ] {
        for count in [Some(12), None] {
            let site = pool_of(&[entry], count, 2020, entry, &[("x", listing, 12)]);
            match walk_over(site).await {
                Ok(p) => assert_eq!(p.hit.slug, "x", "{entry}, {count:?}"),
                Err(e) => panic!("{entry}, {listing}, {count:?}: refused: {e:?}"),
            }
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
        ("［Oshi no Ko］ 2nd Season", "Oshi no Ko"),
        ("［Show］ 2", "Show"),
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

/// A title compared as written carries its catalogue tags wherever
/// they stand: "Show Season 2 (2011)" is the entry "Show (2011) Season
/// 2" exactly, and wins over the original's "Show Season 2" listed
/// first, with a count and without.
#[tokio::test]
async fn a_title_as_written_carries_its_tags_wherever_they_stand() {
    for count in [Some(12), None] {
        let site = pool_of(
            &["Show (2011) Season 2"],
            count,
            2020,
            "Show (2011) Season 2",
            &[
                ("orig", "Show Season 2", 12),
                ("own", "Show Season 2 (2011)", 12),
            ],
        );
        assert_eq!(
            walk_over(site).await.expect("picked").hit.slug,
            "own",
            "{count:?}"
        );
    }
}

/// The probe head's evidence reaches as far as its five: with a count,
/// the hits that cannot be picked are probed as evidence up to the
/// fifth that can, so a broad listing's completing sibling behind four
/// other seasons and the listing itself is probed, and the listing is
/// cut. The sixth such hit is not: a pool still costs at most ten
/// probes, and a sibling past that many is left unread — its listing
/// is not recorded, so reading it fails the test.
#[tokio::test]
async fn the_evidence_reaches_as_far_as_the_five() {
    let site = pool_of(
        &["Show Season 3"],
        Some(12),
        2020,
        "Show Season 3",
        &[
            ("a", "Show Season 2", 12),
            ("b", "Show Season 1", 12),
            ("c", "Show", 12),
            ("d", "Show 2", 12),
            ("e", "Show Season 2 Part 2", 12),
            ("w", "Show Season 3", 24),
            ("l", "Show Season 3 Part 2", 12),
        ],
    );
    let picked = walk_over(site).await.expect("the cut listing");
    assert_eq!(picked.hit.slug, "w");
    assert_eq!(picked.episodes.len(), 12);

    let mut site = pool_of(
        &["Show Season 3"],
        Some(12),
        2020,
        "Show Season 3",
        &[
            ("a", "Show Season 2", 12),
            ("b", "Show Season 1", 12),
            ("d", "Show 2", 12),
            ("e", "Show Season 2 Part 2", 12),
            ("f", "Other Show Season 2", 12),
            ("w", "Show Season 3", 24),
            ("l", "Show Season 3 Part 2", 12),
        ],
    );
    site.doc.episodes.remove("l");
    site.doc.years.remove("l");
    if let Ok(p) = walk_over(site).await {
        panic!("picked {} ({})", p.hit.slug, p.episodes.len());
    }
}

/// Without a count, an entry with a year took every hit its year did
/// not disprove, in the provider's order: a hit a year off (the
/// December allowance) or with no year on its page, however it was
/// named. The year tolerated such a hit without vouching for it. Now a
/// hit is taken only when something vouches for it — the title
/// searched, its own year, or the entry's name under any alias — so a
/// subtitled sequel the provider does not list yet, airing the year
/// after its series, is no longer played as the series. The pool is
/// then the plain miss: nothing — count, year or name — spoke for any
/// hit, so a walk that finds nothing is the clean verdict. A hit the
/// year or the name vouches for is taken from behind one it merely
/// tolerated, and whether the year disproved some other hit in the
/// pool changes nothing.
#[tokio::test]
async fn a_countless_entry_with_a_year_takes_only_a_hit_its_year_or_name_vouches_for() {
    let entry = ["Show: Arc", "Show: The Arc"];
    // Alone in the pool: (the hit's title, its page's year, taken?).
    for (title, year, want) in [
        ("Show: The Arc", Some(2016), true),
        ("Show", Some(2016), true),
        ("Show: The Arc", Some(2015), true),
        ("Show", Some(2015), false),
        ("Show: The Arc", None, true),
        ("Show", None, false),
    ] {
        let site = inline_pool(&entry, None, Some(2016), "TV", &[("x", title, 12, year)]);
        match (walk_over(site).await, want) {
            (Ok(p), true) => assert_eq!(p.hit.slug, "x", "{title} {year:?}"),
            (Err(e), false) => assert!(e.clean_miss, "{title} {year:?}: {:?}", e.error),
            (Ok(p), false) => panic!("{title} {year:?}: picked {}", p.hit.slug),
            (Err(e), true) => panic!("{title} {year:?}: refused: {:?}", e.error),
        }
    }
    // Behind the series the year tolerates, a hit something vouches
    // for is the pick: named, whatever its page year, or unnamed from
    // the entry's own year.
    for (title, year) in [
        ("Show: The Arc", Some(2016)),
        ("Show: The Arc", Some(2015)),
        ("Show: The Arc", None),
        ("Other", Some(2016)),
    ] {
        let site = inline_pool(
            &entry,
            None,
            Some(2016),
            "TV",
            &[("s", "Show", 12, Some(2015)), ("x", title, 12, year)],
        );
        let picked = walk_over(site)
            .await
            .unwrap_or_else(|e| panic!("{title} {year:?} behind the series: {:?}", e.error));
        assert_eq!(picked.hit.slug, "x", "{title} {year:?}");
    }
    // A second hit the year merely tolerates, named by nothing, is no
    // more than the first: the pool is still the clean miss.
    for year in [Some(2015), None] {
        let site = inline_pool(
            &entry,
            None,
            Some(2016),
            "TV",
            &[
                ("s", "Show", 12, Some(2015)),
                ("x", "Show Remake", 12, year),
            ],
        );
        match walk_over(site).await {
            Err(e) => assert!(e.clean_miss, "{year:?}: {:?}", e.error),
            Ok(p) => panic!("{year:?}: picked {}", p.hit.slug),
        }
    }
    // The year disproving another hit changes nothing: a named hit a
    // year off is taken beside one decades off.
    let site = inline_pool(
        &entry,
        None,
        Some(2016),
        "TV",
        &[
            ("o", "Old", 12, Some(1990)),
            ("x", "Show: The Arc", 12, Some(2015)),
        ],
    );
    assert_eq!(walk_over(site).await.expect("named").hit.slug, "x");
}

/// A subtitled sequel the provider does not list yet, announced with a
/// premiere the year after its series and no count on Kitsu: every
/// search for it answers with the series, whose page names the year
/// before. The series is not the entry, and the pool is the clean
/// miss.
#[tokio::test]
async fn a_subtitled_sequel_the_provider_does_not_list_is_not_played_as_its_series() {
    let site = inline_pool(
        &["Show: Arc"],
        None,
        Some(2016),
        "TV",
        &[("show-1", "Show", 12, Some(2015))],
    );
    match walk_over(site).await {
        Ok(p) => panic!("played the series: {}", p.hit.slug),
        Err(e) => assert!(e.clean_miss, "{:?}", e.error),
    }
}

/// [`walk`] under the premiere year AniList names for the entry
/// instead of Kitsu's, which the recording carries: what the pick
/// makes of the same pools once that year is the one it is given.
async fn walk_under_anilist_year(id: &str) -> std::result::Result<PickedShow, NativeError> {
    let site = Recorded::load(id);
    let e = &site.doc.kitsu;
    let year = e
        .anilist_year
        .unwrap_or_else(|| panic!("fixture {id} names no AniList year"));
    pick_native_walk(
        &site,
        &e.canonical,
        &e.alt_titles,
        e.episode_count,
        Some(year),
        e.subtype.as_deref(),
        None,
    )
    .await
}

/// The multi-episode entries Kitsu lists with a count and no premiere
/// date — fourteen on 2026-10-15, the whole of its to-be-announced
/// catalogue that has a count and more than one episode — and what the
/// walk makes of the ones hianime lists or refuses rightly: two are
/// listed under a name one of the entry's titles carries, and four it
/// does not list are the clean miss, every pool empty (Cubistar), its
/// hits ruled out by format before any listing is read (Arpiel's two
/// OVAs for a TV entry, Jidou Bungaku Library's TV series and film for
/// an OVA), or off by more than the count tolerance (Astro Boy Reboot).
const UNDATED_AUDITED_RIGHT: &[Expectation] = &[
    ("47150", Some(("naruto-shinsaku-anime-1984", 4))), // NARUTO×NINKU, listed as "Naruto (Shinsaku Anime)"
    ("50237", Some(("the-outcast-season-6-3813", 26))), // Yi Ren Zhi Xia 6th Season, "The Outcast 6"
    ("11576", None),                                    // Arpiel: 6 Destinies
    ("11944", None),                                    // Astro Boy Reboot
    ("44505", None),                                    // Cubistar
    ("11061", None),                                    // Jidou Bungaku Library
];

#[tokio::test]
async fn undated_entries_the_catalogue_lists_or_lacks_outright_are_picked_right() {
    assert_picks(UNDATED_AUDITED_RIGHT).await;
}

/// Yi Ren Zhi Xia's sixth season is dated on AniList (2026) where Kitsu
/// has no date; its listing's page names the same year, so the pick is
/// the same with the year as without it.
#[tokio::test]
async fn an_undated_entry_anilist_dates_keeps_its_pick_under_that_year() {
    let picked = walk_under_anilist_year("50237").await.expect("picked");
    assert_eq!(picked.hit.slug, "the-outcast-season-6-3813");
}

/// Devil May Cry's third season, with a count and no date: hianime
/// lists the first two seasons alone. The canonical search answers with
/// the second, whose count of eight fits the entry's — refused by name,
/// the pool is the answered dead end rather than the clean miss, since
/// a hit the count accepted was refused on how it is written; the
/// Japanese alias answers with the 2007 series, the entry before, which
/// is never the entry. The walk surfaces the miss without the clean
/// verdict, so the availability cache persists nothing for it.
#[tokio::test]
async fn a_season_the_catalogue_stops_short_of_is_a_dead_end_not_a_miss() {
    match walk("50744").await {
        Ok(p) => panic!("picked {} ({})", p.hit.slug, p.hit.title),
        Err(e) => {
            assert!(!e.clean_miss, "a refusal by name persisted: {:?}", e.error);
            assert!(
                matches!(e.error, crate::error::AniError::NoResults),
                "{:?}",
                e.error
            );
        }
    }
}

/// Each row a Kitsu id whose every pool the walk must refuse, and
/// refuse without the clean verdict: the refusal is an inference from
/// how the hits are named, so it is the answered dead end the
/// availability cache never persists, never the miss it does.
async fn assert_refused_unpersisted(ids: &[&str]) {
    let mut wrong = Vec::new();
    for id in ids {
        match walk(id).await {
            Ok(p) => wrong.push(format!("{id}: picked {} ({})", p.hit.slug, p.hit.title)),
            Err(e) if e.clean_miss => {
                wrong.push(format!("{id}: refused as the clean miss: {:?}", e.error));
            }
            Err(e) if !matches!(e.error, crate::error::AniError::NoResults) => {
                wrong.push(format!("{id}: {:?}", e.error));
            }
            Err(e) if !e.refused_by_name => {
                wrong.push(format!("{id}: not reported as a refusal by name"));
            }
            Err(_) => {}
        }
    }
    assert!(wrong.is_empty(), "wrong verdicts:\n{}", wrong.join("\n"));
}

/// The other seven undated entries hianime does not list under any of
/// their titles — its searches answer with other shows, and one of them
/// sits within the count tolerance of the entry: a hentai OVA two
/// episodes long for a five-episode idol OVA, a twelve-episode romance
/// for a twelve-episode donghua. With a count and no year, the count is
/// all that vouched for a hit, so each was played as the entry. Nothing
/// but its count fits any of them — not the title searched, not a name
/// of the entry's, not a year — and every pool should be refused: the
/// answered dead end, since a hit the count accepted is refused on how
/// it is named, which the availability cache does not persist.
const UNDATED_REFUSED: &[&str] = &[
    "8404",  // Tsubu★Doll (5): picked a 2-episode OVA
    "43745", // Chuan Shu Zijiu Zhinan 2nd Season (10): picked Fei Ren Zai 2nd Season (12)
    "45637", // Mo Du Zhe (12): picked Girlfriend, Girlfriend (12)
    "45049", // Lady Napoleon (13): picked a 12-episode isekai
    "45494", // Liu Yao (13): picked Zhen Dao Ge: Qian li bu Liu Xing (12)
    "40684", // Ohanashi de Te Koi (5): picked a 4-episode hentai OVA
    "45500", // Zai Xia Ying Tai (12): picked Hero Return (12)
];

#[tokio::test]
async fn undated_entries_the_catalogue_does_not_list_are_refused() {
    assert_refused_unpersisted(UNDATED_REFUSED).await;
}

/// Zai Xia Ying Tai is the one of the seven AniList dates (2025) where
/// Kitsu does not. Given that year the walk is a dated one: Hero Return
/// (2020) and every other dated hit of the first pool are excluded, and
/// the alias pool holds a twelve-episode show from 2025 — year-confirmed,
/// count fitting — that no name vouches for: not its title, not its
/// card's romanized name, not the Japanese title its page gives
/// (不遇職【鑑定士】が実は最強だった). A dated entry with a count takes only
/// a hit a name vouches for, so the walk refuses it, as the dead end a
/// refusal by name is.
#[tokio::test]
async fn a_dated_walk_over_the_same_pools_refuses_the_unnamed_fit() {
    match walk_under_anilist_year("45500").await {
        Ok(p) => panic!("picked {} ({})", p.hit.slug, p.hit.title),
        Err(e) => {
            assert!(!e.clean_miss, "a refusal by name persisted");
            assert!(
                matches!(e.error, crate::error::AniError::NoResults),
                "{:?}",
                e.error
            );
            // The pool's Fei Ren Zai parts are refused by the entry's
            // titles and probed only as evidence; that their pages list
            // no genre leaves the refusal what it is.
            assert!(e.refused_by_name, "not the refusal by name: {e:?}");
        }
    }
}

/// Dated entries hianime does not list under any of their titles,
/// whose searches answer with a same-year show of the right length —
/// one year off, which the year filter tolerates as a December
/// premiere. Each played as the entry on its year and count alone;
/// nothing names either: HenSemi (2010, two episodes) took the
/// two-episode Zan Sayonara Zetsubou Sensei Bangaichi (2009) from the
/// pool its own title answers with, HenSemi (TV) (2011, 13) took Medaka
/// Box Abnormal (2012, 12), which its alias "Abnormal Physiology
/// Seminar" surfaced. Both are refused as the dead end a refusal by
/// name is, which the availability cache does not persist.
const DATED_REFUSED: &[&str] = &[
    "5196", // HenSemi (2): picked Zan Sayonara Zetsubou Sensei Bangaichi (2)
    "6067", // HenSemi (TV) (13): picked Medaka Box Abnormal (12)
];

#[tokio::test]
async fn dated_entries_the_catalogue_does_not_list_are_refused() {
    assert_refused_unpersisted(DATED_REFUSED).await;
}

/// A hit its own entry page files under the genre Hentai is never a
/// pick: the app reads Kitsu anonymously, and Kitsu serves no adult
/// entry that way, so no entry the app shows is one. Ohanashi de Te Koi
/// (40684) and Tsubu★Doll (8404) played hentai OVAs on master, and the
/// count-alone rule is what refuses them as recorded
/// (`UNDATED_REFUSED`); the hentai's page is recorded beside them. Here
/// each entry is given the hentai's title among its names — so its
/// count and a name vouch for it — and, dated, the hentai's own
/// premiere year besides. The walk still takes nothing, dated or not:
/// a dated entry's year filter reads the hit's page and drops it, and
/// an undated entry's pick reads the page of the listing it would serve
/// and is made again without it. Either way the pools hold nothing
/// else the entry could be — the clean miss a pool of other years'
/// shows is. With the page listing another genre in its place, the
/// same walk takes the hentai. The same holds with the entry's count
/// taken away.
#[tokio::test]
async fn a_hit_filed_under_hentai_is_never_picked() {
    for (id, slug) in [
        (
            "40684",
            "katainaka-ni-totsuidekita-ssia-musume-to-h-shimakuru-ohanashi-2166",
        ),
        (
            "8404",
            "muttsuri-dosukebe-tsuyu-gibo-shimai-no-honshitsu-minuite-sex-sanmai-2256",
        ),
    ] {
        let vouched_for = |hentai: bool, dated: bool, counted: bool| {
            let mut site = Recorded::load(id);
            let (query, title) = site
                .doc
                .searches
                .iter()
                .find_map(|(q, hits)| {
                    let (_, title, _) = hits.iter().find(|(s, _, _)| s == slug)?;
                    Some((q.clone(), title.clone()))
                })
                .unwrap_or_else(|| panic!("{id}: no search answers with {slug}"));
            let pool = site.doc.searches[&query].clone();
            site.doc.searches.insert(title.clone(), pool);
            site.doc.kitsu.alt_titles.push(title);
            if dated {
                site.doc.kitsu.year = site.doc.years[slug];
            }
            if !counted {
                site.doc.kitsu.episode_count = None;
            }
            if !hentai {
                site.doc
                    .genres
                    .insert(slug.to_string(), vec![LISTED_GENRE.to_string()]);
            }
            site
        };
        for dated in [true, false] {
            // With its count and without one: every path drops the
            // hit, a countless entry's included.
            for counted in [true, false] {
                let case = format!("{id} dated {dated} counted {counted}");
                match walk_over(vouched_for(true, dated, counted)).await {
                    Ok(p) => panic!("{case}: picked {} ({})", p.hit.slug, p.hit.title),
                    // The page says the listing is not the entry's, as
                    // a year off it would, and nothing else in either
                    // pool is named for the entry: the clean miss.
                    Err(e) => assert!(e.clean_miss, "{case}: {:?}", e.error),
                }
                let control = walk_over(vouched_for(false, dated, counted))
                    .await
                    .unwrap_or_else(|e| panic!("{case}: the control refused: {e:?}"));
                assert_eq!(control.hit.slug, slug, "{case}: the control's pick");
            }
        }
    }
}

/// What the year and count would have accepted is more than a hit
/// within the tolerance: a short listing from the entry's year is
/// rescued as an airing part, and same-year parts of a split are
/// stitched. A dated entry's candidate no name vouches for is refused
/// in either shape too, and that refusal is by name, never the clean
/// miss the availability cache persists; a pool whose unnamed hit
/// nothing would have taken — longer than the entry by more than the
/// tolerance — is still the clean miss it was.
#[tokio::test]
async fn a_refused_rescue_or_split_is_not_a_clean_miss() {
    let y = Some(2020);
    for (label, count, rows) in [
        ("an airing part", 12, vec![("x", "Other", 2, y)]),
        (
            "a split",
            24,
            vec![("a", "Other", 12, y), ("b", "Other Part 2", 12, y)],
        ),
    ] {
        match walk_over(inline_pool(&["Show"], Some(count), Some(2020), "TV", &rows)).await {
            Ok(p) => panic!("{label}: picked {}", p.hit.slug),
            Err(e) => assert!(!e.clean_miss, "{label}: a refusal by name persisted"),
        }
    }
    let longer = inline_pool(
        &["Show"],
        Some(12),
        Some(2020),
        "TV",
        &[("x", "Other", 40, y)],
    );
    match walk_over(longer).await {
        Ok(p) => panic!("longer: picked {}", p.hit.slug),
        Err(e) => assert!(e.clean_miss, "longer: {:?}", e.error),
    }
}

/// What vouches for a hit depends on what the entry knows. An entry
/// with a count takes only a hit a name vouches for, dated or not: a
/// year and a count fit many shows as a count alone does, so a hit
/// neither the title searched nor one of the entry's names vouches for
/// is refused, as the dead end a refusal by name is — whether its page
/// names the entry's year or none — and a hit an alias names is still
/// taken on its count. Without a count the pool is refused as before,
/// and as the clean miss it was.
#[tokio::test]
async fn an_entry_with_a_count_takes_only_a_hit_named_for_it() {
    let other = |year: Option<u32>| ("other-1", "Other", 12, year);
    for (label, year) in [
        ("dated, year confirmed", Some(2016)),
        ("dated, year unknown", None),
    ] {
        let dated = inline_pool(&["Show: Arc"], Some(12), Some(2016), "TV", &[other(year)]);
        match walk_over(dated).await {
            Ok(p) => panic!("{label}: picked {}", p.hit.slug),
            Err(e) => {
                assert!(!e.clean_miss, "{label}: a refusal by name persisted");
                assert!(
                    matches!(e.error, crate::error::AniError::NoResults),
                    "{label}: {:?}",
                    e.error
                );
            }
        }
    }
    let dated_named = inline_pool(
        &["Show: Arc", "Shou Arc"],
        Some(12),
        Some(2016),
        "TV",
        &[other(Some(2016)), ("shou-2", "Shou Arc", 12, Some(2016))],
    );
    assert_eq!(
        walk_over(dated_named).await.expect("dated, named").hit.slug,
        "shou-2"
    );
    let undated = inline_pool(&["Show: Arc"], Some(12), None, "TV", &[other(None)]);
    match walk_over(undated).await {
        Ok(p) => panic!("undated: picked {}", p.hit.slug),
        Err(e) => {
            assert!(!e.clean_miss, "undated: a refusal by name persisted");
            assert!(
                matches!(e.error, crate::error::AniError::NoResults),
                "undated: {:?}",
                e.error
            );
        }
    }
    let named = inline_pool(
        &["Show: Arc", "Shou Arc"],
        Some(12),
        None,
        "TV",
        &[other(None), ("shou-2", "Shou Arc", 12, None)],
    );
    assert_eq!(
        walk_over(named).await.expect("undated, named").hit.slug,
        "shou-2"
    );
    let countless = inline_pool(&["Show: Arc"], None, None, "TV", &[other(None)]);
    match walk_over(countless).await {
        Ok(p) => panic!("countless: picked {}", p.hit.slug),
        Err(e) => assert!(e.clean_miss, "countless: {:?}", e.error),
    }
}
