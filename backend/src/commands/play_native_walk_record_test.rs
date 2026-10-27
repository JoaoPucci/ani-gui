//! Recording the picker fixtures: the walk run against hianime through
//! the impersonating transport, every search, listing and entry page
//! it read reduced to what the client's parsers return. Both tests
//! here are ignored and run by hand; they write fixtures, they assert
//! nothing about the picker.
//!
//! Every answer the site gives is kept on disk under
//! `PICKER_RECORD_PAGES`, keyed by the request, and a request already
//! there is answered from it, so a recording can be re-run, extended
//! or reduced again without asking the site twice. With
//! `PICKER_RECORD_BIN` naming the directory that holds the bundled
//! transport, a request not on disk is made live — one at a time, at
//! least [`PACING`] apart, and the run stops on the second refusal
//! (403 or 429) in a row; without it, such a request fails the run.
//!
//! Run from `backend/`, where the test binary runs too, so a relative
//! path resolves there; the paths below are absolute for that reason,
//! `$ROOT` the repository's root and `$PAGES` any directory outside it.
//!
//! ```sh
//! cd "$ROOT/backend"
//! # A new fixture: the entry's arguments as the app sends them, as a
//! # recording's `kitsu` block
//! PICKER_RECORD_ENTRY="$PAGES/entry.json" PICKER_RECORD_PAGES="$PAGES" \
//!   PICKER_RECORD_BIN="$ROOT/electron/.linux-deps-cache/extract-curl-impersonate" \
//!   PICKER_RECORD_OUT="$ROOT/tests/fixtures/hianime/picker/<id>.json" \
//!   cargo test --lib record_a_picker_fixture -- --ignored
//! # The cards' romanized names and the pages' names and genres of
//! # every recorded fixture
//! PICKER_RECORD_PAGES="$PAGES" \
//!   PICKER_RECORD_BIN="$ROOT/electron/.linux-deps-cache/extract-curl-impersonate" \
//!   cargo test --lib record_the_page_names -- --ignored
//! ```

use super::*;
use crate::scraper::fetch::{CurlImpersonateFetch, Fetch, FetchRequest, FetchResponse};
use crate::scraper::hianime::HianimeClient;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The least time between two live requests.
const PACING: std::time::Duration = std::time::Duration::from_millis(3200);

/// The site's answers on disk, and the transport that fetches what is
/// not there yet.
struct Pages {
    dir: PathBuf,
    live: Option<CurlImpersonateFetch>,
    last: Mutex<Option<std::time::Instant>>,
    refusals: Mutex<u32>,
}

impl Pages {
    fn from_env() -> Self {
        let dir = PathBuf::from(std::env::var("PICKER_RECORD_PAGES").expect("PICKER_RECORD_PAGES"));
        std::fs::create_dir_all(&dir).expect("pages dir");
        let live = std::env::var("PICKER_RECORD_BIN").ok().map(|bin| {
            CurlImpersonateFetch::resolve(Some(Path::new(&bin)), "")
                .expect("PICKER_RECORD_BIN holds the bundled transport")
        });
        Self {
            dir,
            live,
            last: Mutex::new(None),
            refusals: Mutex::new(0),
        }
    }

    /// Where the answer to `req` is kept: the request's URL and
    /// headers, hashed.
    fn path_of(&self, req: &FetchRequest) -> PathBuf {
        self.dir.join(format!("{}.page", key_of(req)))
    }

    async fn paced(&self) {
        let wait = self
            .last
            .lock()
            .expect("pacing")
            .map(|at| PACING.saturating_sub(at.elapsed()));
        if let Some(wait) = wait {
            tokio::time::sleep(wait).await;
        }
        *self.last.lock().expect("pacing") = Some(std::time::Instant::now());
    }
}

/// The key a request's answer is kept under.
fn key_of(req: &FetchRequest) -> String {
    let mut hash = Sha256::new();
    hash.update(req.url.as_bytes());
    for (name, value) in &req.headers {
        hash.update(format!("\n{name}: {value}").as_bytes());
    }
    format!("{:x}", hash.finalize())
}

#[async_trait::async_trait]
impl Fetch for Pages {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        let path = self.path_of(req);
        if let Ok(kept) = std::fs::read_to_string(&path) {
            let (head, body) = kept.split_once('\n').expect("kept page head");
            let (status, url) = head.split_once(' ').expect("kept page status");
            return Ok(FetchResponse {
                status: status.parse().expect("kept status"),
                body: body.to_string(),
                url: url.to_string(),
            });
        }
        let live = self
            .live
            .as_ref()
            .unwrap_or_else(|| panic!("no page kept for {} and no live transport", req.url));
        self.paced().await;
        let resp = live.fetch(req).await?;
        eprintln!("live {} {}", resp.status, req.url);
        let mut refusals = self.refusals.lock().expect("refusals");
        if matches!(resp.status, 403 | 429) {
            *refusals += 1;
            assert!(*refusals < 2, "the site refused twice in a row: stopping");
            return Ok(resp);
        }
        *refusals = 0;
        std::fs::write(
            &path,
            format!("{} {}\n{}", resp.status, resp.url, resp.body),
        )
        .expect("keep page");
        Ok(resp)
    }
}

/// What the walk read, as the parsers returned it.
#[derive(Default)]
struct Reads {
    searches: BTreeMap<String, Vec<BrowseHit>>,
    episodes: BTreeMap<String, Vec<EpisodeRef>>,
    pages: BTreeMap<String, EntryPage>,
}

/// The client, every answer it parses noted.
struct Recording<P> {
    inner: P,
    reads: Mutex<Reads>,
}

#[async_trait::async_trait]
impl<P: Provider> Provider for Recording<P> {
    fn id(&self) -> ProviderId {
        self.inner.id()
    }
    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        let hits = self.inner.search(query).await?;
        let mut reads = self.reads.lock().expect("reads");
        reads.searches.insert(query.to_string(), hits.clone());
        Ok(hits)
    }
    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        let rows = self.inner.episodes(slug).await?;
        let mut reads = self.reads.lock().expect("reads");
        reads.episodes.insert(slug.to_string(), rows.clone());
        Ok(rows)
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
        self.detail_page(slug).await.map(|page| page.year)
    }
    async fn detail_page(&self, slug: &str) -> Result<EntryPage> {
        let page = self.inner.detail_page(slug).await?;
        let mut reads = self.reads.lock().expect("reads");
        reads.pages.insert(slug.to_string(), page.clone());
        Ok(page)
    }
    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        None
    }
}

fn client() -> Recording<HianimeClient<Pages>> {
    Recording {
        inner: HianimeClient::new(Pages::from_env()),
        reads: Mutex::new(Reads::default()),
    }
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("serialize")
}

/// An array written as the recordings write one: on one line, its
/// items apart.
fn row(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

/// A list of names, as a row.
fn names_row(names: &[String]) -> String {
    row(&names.iter().map(json).collect::<Vec<_>>())
}

/// A map's body in braces, as the recordings write a map: an empty
/// one on one line.
fn braced(body: &str) -> String {
    if body.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{body}\n }}")
    }
}

/// One `"key": value` line per entry, at the recordings' indentation.
fn lines<'a>(rows: impl Iterator<Item = (&'a String, String)>) -> String {
    rows.map(|(k, v)| format!("  {}: {v}", json(k)))
        .collect::<Vec<_>>()
        .join(",\n")
}

/// The maps a recording ends with — the cards' romanized names, and
/// the names, genres and rating every entry page read gives — written
/// as the recordings write their maps.
fn name_maps(reads: &Reads, hits: impl Iterator<Item = BrowseHit>) -> String {
    let jnames: BTreeMap<String, String> =
        hits.filter_map(|h| h.jname.map(|j| (h.slug, j))).collect();
    let names = reads
        .pages
        .iter()
        .filter(|(_, p)| !p.names.is_empty())
        .map(|(s, p)| (s, names_row(&p.names)));
    let genres = reads
        .pages
        .iter()
        .filter(|(_, p)| !p.genres.is_empty())
        .map(|(s, p)| (s, names_row(&p.genres)));
    let ratings = reads
        .pages
        .iter()
        .filter_map(|(s, p)| Some((s, json(p.rating.as_ref()?))));
    format!(
        " \"jnames\": {},\n \"page_names\": {},\n \"genres\": {},\n \"ratings\": {}\n",
        braced(&lines(jnames.iter().map(|(s, j)| (s, json(j))))),
        braced(&lines(names)),
        braced(&lines(genres)),
        braced(&lines(ratings)),
    )
}

/// A recording of the entry `PICKER_RECORD_ENTRY` names — its Kitsu
/// arguments as the app sends them, as a recording's `kitsu` block —
/// over every title it goes by: each title is searched, and the pick
/// is made over each pool that answers, whether or not an earlier
/// pool already picked, so any walk over the entry finds what it
/// reads in the recording.
#[tokio::test]
#[ignore = "records a fixture from the live site; run by hand"]
async fn record_a_picker_fixture() {
    let entry_path = std::env::var("PICKER_RECORD_ENTRY").expect("PICKER_RECORD_ENTRY");
    let kitsu: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&entry_path).expect("entry"))
            .expect("entry json");
    let e: Entry = serde_json::from_value(kitsu.clone()).expect("entry args");
    let site = client();
    let titles: Vec<&str> = std::iter::once(e.canonical.as_str())
        .chain(e.alt_titles.iter().map(String::as_str))
        .collect();
    for t in &titles {
        let hits = site.search(t).await.expect("search");
        if hits.is_empty() {
            continue;
        }
        let picked = pick_candidate_titled(
            &site,
            &hits,
            e.episode_count,
            t,
            &titles,
            e.year,
            e.subtype.as_deref(),
        )
        .await;
        eprintln!("{t:?}: {:?}", picked.map(|p| p.hit.slug));
    }
    let reads = site.reads.into_inner().expect("reads");
    let block = |rows: Vec<String>| rows.join(",\n");
    let searches = block(
        reads
            .searches
            .iter()
            .map(|(q, hits)| {
                let rows: Vec<String> = hits
                    .iter()
                    .map(|h| {
                        format!(
                            "    {}",
                            row(&[json(&h.slug), json(&h.title), json(&h.kind)])
                        )
                    })
                    .collect();
                if rows.is_empty() {
                    format!("  {}: []", json(q))
                } else {
                    format!("  {}: [\n{}\n  ]", json(q), rows.join(",\n"))
                }
            })
            .collect(),
    );
    let episodes = block(
        reads
            .episodes
            .iter()
            .map(|(slug, rows)| {
                let rows: Vec<String> = rows
                    .iter()
                    .map(|r| {
                        format!(
                            "    {}",
                            row(&[json(&r.id), json(&r.number), json(&r.number2)])
                        )
                    })
                    .collect();
                format!("  {}: [\n{}\n  ]", json(slug), rows.join(",\n"))
            })
            .collect(),
    );
    let years = lines(reads.pages.iter().map(|(s, p)| (s, json(&p.year))));
    // The arguments in the order every recording writes them.
    const ARGS: [&str; 8] = [
        "id",
        "name",
        "canonical",
        "alt_titles",
        "episode_count",
        "year",
        "anilist_year",
        "subtype",
    ];
    let args = kitsu.as_object().expect("entry object");
    let kitsu_block = ARGS
        .iter()
        .filter_map(|k| {
            let v = args.get(*k)?;
            let v = match v {
                serde_json::Value::Array(items) => row(&items.iter().map(json).collect::<Vec<_>>()),
                other => json(other),
            };
            Some(format!("  {}: {v}", json(k)))
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let hits = reads
        .searches
        .values()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let out = format!(
        "{{\n \"kitsu\": {},\n \"searches\": {},\n \"episodes\": {},\n \"years\": {},\n{}}}\n",
        braced(&kitsu_block),
        braced(&searches),
        braced(&episodes),
        braced(&years),
        name_maps(&reads, hits.into_iter()),
    );
    let out_path = std::env::var("PICKER_RECORD_OUT").expect("PICKER_RECORD_OUT");
    std::fs::write(&out_path, out).expect("write fixture");
    // The recording reads back as the fixture tests read it.
    let _: Fixture = serde_json::from_str(&std::fs::read_to_string(&out_path).expect("reread"))
        .expect("the recording parses as a fixture");
}

/// Every recorded fixture given the romanized names its search cards
/// carry and the names, genres and ratings its entry pages give: each
/// recorded search and each recorded entry page is read again and
/// reduced by the same parsers, and the four maps are written after the
/// recording's own, which are left byte for byte as they were. A
/// search that no longer answers with the recorded cards, or a page
/// that no longer names the recorded year, is reported; the names of
/// the cards still there are kept, and the recorded pool itself is
/// never rewritten.
#[tokio::test]
#[ignore = "re-reads every recorded search and entry page; run by hand"]
async fn record_the_page_names() {
    let mut ids: Vec<String> = std::fs::read_dir(fixture_dir())
        .expect("fixture dir")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            id.chars()
                .all(|c| c.is_ascii_digit())
                .then(|| id.to_string())
        })
        .collect();
    ids.sort();
    let site = client();
    for id in ids {
        let path = fixture_dir().join(format!("{id}.json"));
        let text = std::fs::read_to_string(&path).expect("fixture");
        if text.contains("\n \"jnames\": {") {
            continue;
        }
        let doc: Fixture = serde_json::from_str(&text).expect("fixture parses");
        *site.reads.lock().expect("reads") = Reads::default();
        let mut cards = Vec::new();
        for (q, recorded) in &doc.searches {
            let hits = site.search(q).await.expect("search");
            let now: Vec<(&str, &str, Option<&str>)> = hits
                .iter()
                .map(|h| (h.slug.as_str(), h.title.as_str(), h.kind.as_deref()))
                .collect();
            let then: Vec<(&str, &str, Option<&str>)> = recorded
                .iter()
                .map(|(s, t, k)| (s.as_str(), t.as_str(), k.as_deref()))
                .collect();
            if now != then {
                eprintln!("{id}: the search for {q:?} answers otherwise now");
            }
            cards.extend(
                hits.into_iter()
                    .filter(|h| recorded.iter().any(|(s, _, _)| *s == h.slug)),
            );
        }
        for (slug, year) in &doc.years {
            let page = site.detail_page(slug).await.expect("entry page");
            if page.year != *year {
                eprintln!("{id}: {slug} names {:?} now, recorded {year:?}", page.year);
            }
        }
        let reads = std::mem::take(&mut *site.reads.lock().expect("reads"));
        let body = text
            .strip_suffix("\n}\n")
            .expect("a recording ends on its brace");
        let out = format!("{body},\n{}}}\n", name_maps(&reads, cards.into_iter()));
        std::fs::write(&path, out).expect("write fixture");
        let _: Fixture = serde_json::from_str(&std::fs::read_to_string(&path).expect("reread"))
            .expect("the recording parses as a fixture");
    }
}

/// A recording that asks the live site for the entry pages it does not
/// hold, and answers everything else as recorded.
struct Filling {
    recorded: Recorded,
    live: Recording<HianimeClient<Pages>>,
}

#[async_trait::async_trait]
impl Provider for Filling {
    fn id(&self) -> ProviderId {
        self.recorded.id()
    }
    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        self.recorded.search(query).await
    }
    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        self.recorded.episodes(slug).await
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
        self.detail_page(slug).await.map(|page| page.year)
    }
    async fn detail_page(&self, slug: &str) -> Result<EntryPage> {
        if self.recorded.doc.years.contains_key(slug) {
            self.recorded.detail_page(slug).await
        } else {
            self.live.detail_page(slug).await
        }
    }
    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        None
    }
}

/// `text` with `"key": value` written into its map `map`, the map's
/// lines kept in key order, as the recordings write their maps.
fn with_line(text: &str, map: &str, key: &str, value: &str) -> String {
    let head = format!("\n \"{map}\": ");
    let at = text.find(&head).expect("the recording's map") + head.len();
    let rest = &text[at..];
    let end = if rest.starts_with("{}") {
        2
    } else {
        rest.find("\n }").expect("the map's closing brace") + 3
    };
    let mut rows: Vec<String> = rest[..end]
        .trim_start_matches('{')
        .trim_end_matches('}')
        .lines()
        .map(|l| l.trim_end_matches(',').to_string())
        .filter(|l| !l.trim().is_empty())
        .collect();
    rows.push(format!("  {}: {value}", json(&key)));
    rows.sort();
    format!(
        "{}{}{}",
        &text[..at],
        braced(&rows.join(",\n")),
        &rest[end..]
    )
}

/// `text` with `"key": value` written into its ratings map, the map
/// added after the genres where a recording from before ratings were
/// read has none.
fn with_ratings_line(text: &str, key: &str, value: &str) -> String {
    if text.contains("\n \"ratings\": ") {
        return with_line(text, "ratings", key, value);
    }
    let body = text
        .strip_suffix("\n}\n")
        .expect("a recording ends on its brace");
    let added = format!("{body},\n \"ratings\": {{}}\n}}\n");
    with_line(&added, "ratings", key, value)
}

/// Every recorded fixture given the entry pages its walk reads that the
/// recording does not hold — the page of the listing an undated entry's
/// pick would serve, which the walk did not read when the fixture was
/// recorded. Each fixture's walk runs as the fixture tests run it, under
/// AniList's year too where the recording notes one; the pages it lacks
/// are read live, reduced by the client's parsers and written into the
/// recording's maps, and the rest of the recording is left byte for
/// byte as it was.
#[tokio::test]
#[ignore = "reads the entry pages the recordings lack from the live site; run by hand"]
async fn record_the_pages_the_walk_reads() {
    let mut ids: Vec<String> = std::fs::read_dir(fixture_dir())
        .expect("fixture dir")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            id.chars()
                .all(|c| c.is_ascii_digit())
                .then(|| id.to_string())
        })
        .collect();
    ids.sort();
    for id in ids {
        let site = Filling {
            recorded: Recorded::load(&id),
            live: client(),
        };
        let e = &site.recorded.doc.kitsu;
        let years = [Some(e.year), e.anilist_year.map(Some)];
        for year in years.into_iter().flatten() {
            let _ = pick_native_walk(
                &site,
                &e.canonical,
                &e.alt_titles,
                e.episode_count,
                year,
                e.subtype.as_deref(),
                None,
            )
            .await;
        }
        let reads = site.live.reads.into_inner().expect("reads");
        if reads.pages.is_empty() {
            continue;
        }
        let path = fixture_dir().join(format!("{id}.json"));
        let mut text = std::fs::read_to_string(&path).expect("fixture");
        for (slug, page) in &reads.pages {
            eprintln!("{id}: {slug} {page:?}");
            text = with_line(&text, "years", slug, &json(&page.year));
            if !page.names.is_empty() {
                text = with_line(&text, "page_names", slug, &names_row(&page.names));
            }
            if !page.genres.is_empty() {
                text = with_line(&text, "genres", slug, &names_row(&page.genres));
            }
            if let Some(rating) = &page.rating {
                text = with_ratings_line(&text, slug, &json(rating));
            }
        }
        std::fs::write(&path, text).expect("write fixture");
        let _: Fixture = serde_json::from_str(&std::fs::read_to_string(&path).expect("reread"))
            .expect("the recording parses as a fixture");
    }
}
