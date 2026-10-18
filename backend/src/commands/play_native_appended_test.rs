//! The appended-extras rule against the recorded listings that carry
//! appended extras, against every way the evidence can fall short,
//! and against the corpus of shows where nothing may move.

use super::*;
use crate::commands::play_native_numbering::{
    extra_episode_tags, kitsu_episode_cap, numbering_offset,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/hianime/appended")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The recorded hianime listing for provider show `id`.
fn listing(id: u32) -> Vec<EpisodeRef> {
    let body = String::from_utf8(fixture(&format!("episode-list-{id}.json"))).expect("utf-8");
    crate::scraper::hianime::parse_episode_list(&body).expect("recorded listing")
}

/// The tail Kitsu entry `kitsu` and AniList's answer for it give, as
/// recorded: Kitsu's status, count and first episode page, AniList's
/// finished count.
fn recorded_tail(kitsu: u32) -> EntryTail {
    let detail =
        crate::meta::kitsu::parse_anime_response(&fixture(&format!("kitsu-anime-{kitsu}.json")))
            .expect("recorded entry");
    let page = crate::meta::kitsu::parse_episodes_response(&fixture(&format!(
        "kitsu-episodes-{kitsu}-p1.json"
    )))
    .expect("recorded page");
    let airing = crate::meta::anilist_airing::parse_airing_response(&fixture(&format!(
        "anilist-airing-{kitsu}.json"
    )))
    .expect("recorded airing")
    .expect("AniList knows it");
    EntryTail {
        finished: detail.status.as_deref() == Some("finished"),
        count: detail.episode_count,
        anilist_count: airing.aired.filter(|_| airing.next_episode.is_none()),
        episodes: page
            .into_iter()
            .filter_map(|e| {
                Some(TailEpisode {
                    number: e.number?,
                    title: e.canonical_title,
                    airdate: e.airdate,
                })
            })
            .collect(),
        read_past_count: true,
        related: recorded_related(kitsu),
        related_complete: true,
    }
}

/// The entries Kitsu relates `kitsu` to, as recorded, each TV entry
/// carrying its first episode page where that page is recorded too.
fn recorded_related(kitsu: u32) -> Vec<Related> {
    let doc: serde_json::Value =
        serde_json::from_slice(&fixture(&format!("kitsu-relations-{kitsu}.json")))
            .expect("recorded relations");
    let included = doc["included"].as_array().cloned().unwrap_or_default();
    doc["data"]
        .as_array()
        .expect("relations")
        .iter()
        .map(|r| {
            let dst = &r["relationships"]["destination"]["data"];
            let attrs = included
                .iter()
                .find(|i| i["type"] == dst["type"] && i["id"] == dst["id"])
                .map(|i| i["attributes"].clone())
                .unwrap_or_default();
            let role = r["attributes"]["role"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let subtype = attrs["subtype"].as_str().map(str::to_string);
            let tv = dst["type"] == "anime" && subtype.as_deref() == Some("TV");
            let id = dst["id"].as_str().unwrap_or_default();
            let page = format!("kitsu-episodes-{id}-p1.json");
            Related {
                role,
                anime: dst["type"] == "anime",
                subtype,
                status: attrs["status"].as_str().map(str::to_string),
                episode_count: attrs["episodeCount"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok()),
                first_episodes: tv.then(|| {
                    crate::meta::kitsu::parse_episodes_response(&fixture(&page))
                        .expect("sequel page")
                        .into_iter()
                        .filter_map(|e| {
                            Some(SequelEpisode {
                                number: e.number?,
                                title: e.canonical_title,
                            })
                        })
                        .collect()
                }),
            }
        })
        .collect()
}

/// The tags `rows` carry, in order.
fn tags(rows: &[EpisodeRef]) -> Vec<Option<&str>> {
    rows.iter().map(|e| e.number2.as_deref()).collect()
}

// ── the recorded shows ─────────────────────────────────────────────

/// Noragami: twelve episodes, then the two OADs hianime lists as rows
/// 13 and 14. Kitsu and AniList both end the show at twelve, and
/// Kitsu's last three titles are the listing's, so the OADs become
/// `12.1` and `12.2` beside episode 12 — each still the row it was,
/// slot, id and title — and the cap is twelve.
#[test]
fn noragami_s_oads_become_extras_beside_its_last_episode() {
    let rows = listing(916);
    assert_eq!(appended_count(&rows, Some(12)), Some(12));
    let fitted = appended_extras(&rows, 12, &recorded_tail(7881)).expect("appended extras");
    assert_eq!(fitted.len(), rows.len());
    for (before, after) in rows.iter().zip(&fitted) {
        assert_eq!(
            (before.id, before.number, &before.title),
            (after.id, after.number, &after.title)
        );
    }
    assert!(fitted[..12].iter().all(|e| e.number2.is_none()));
    assert_eq!(tags(&fitted[12..]), vec![Some("12.1"), Some("12.2")]);
    assert_eq!(kitsu_episode_cap(&fitted), Some(12));
    assert_eq!(extra_episode_tags(&fitted), vec!["12.1", "12.2"]);
    assert_eq!(numbering_offset(&fitted), 0);
}

/// Noragami Aragoto ends at thirteen and lists two OADs after it.
#[test]
fn noragami_aragoto_s_oads_become_extras_beside_its_last_episode() {
    let rows = listing(913);
    let fitted = appended_extras(&rows, 13, &recorded_tail(10800)).expect("appended extras");
    assert_eq!(tags(&fitted[13..]), vec![Some("13.1"), Some("13.2")]);
    assert_eq!(kitsu_episode_cap(&fitted), Some(13));
}

/// Satsuriku no Tenshi's four web-only episodes follow its twelve
/// broadcast ones; Kitsu and AniList count the twelve.
#[test]
fn satsuriku_no_tenshi_s_web_episodes_become_extras_beside_its_last_episode() {
    let rows = listing(1549);
    let fitted = appended_extras(&rows, 12, &recorded_tail(13715)).expect("appended extras");
    assert_eq!(
        tags(&fitted[12..]),
        vec![Some("12.1"), Some("12.2"), Some("12.3"), Some("12.4")]
    );
    assert_eq!(kitsu_episode_cap(&fitted), Some(12));
}

// ── the listing's shape ────────────────────────────────────────────

fn plain(titles: &[&str]) -> Vec<EpisodeRef> {
    titles
        .iter()
        .zip(1u32..)
        .map(|(t, n)| EpisodeRef {
            id: u64::from(n) + 100,
            number: n,
            number2: None,
            title: Some((*t).to_string()),
        })
        .collect()
}

#[test]
fn only_a_listing_past_the_count_by_a_few_untagged_rows_asks() {
    let rows = listing(916);
    assert_eq!(appended_count(&rows, None), None, "no count");
    assert_eq!(appended_count(&rows, Some(0)), None, "a count of none");
    assert_eq!(appended_count(&rows, Some(14)), None, "the count");
    assert_eq!(appended_count(&rows, Some(20)), None, "short of the count");
    assert_eq!(appended_count(&rows, Some(13)), Some(13));
    assert_eq!(appended_count(&rows, Some(5)), Some(5), "nine past");
    assert_eq!(appended_count(&rows, Some(4)), None, "ten past");
    let mut tagged = rows.clone();
    tagged[3].number2 = Some("3.5".into());
    assert_eq!(appended_count(&tagged, Some(12)), None, "a tagged listing");
    let mut gapped = rows.clone();
    gapped[5].number = 7;
    assert_eq!(
        appended_count(&gapped, Some(12)),
        None,
        "slots out of order"
    );
    assert_eq!(appended_count(&[], Some(12)), None, "nothing listed");
}

// ── evidence falling short ─────────────────────────────────────────

fn noragami() -> (Vec<EpisodeRef>, EntryTail) {
    (listing(916), recorded_tail(7881))
}

#[test]
fn a_show_kitsu_does_not_call_finished_keeps_its_listing() {
    let (rows, mut tail) = noragami();
    tail.finished = false;
    assert_eq!(appended_extras(&rows, 12, &tail), None);
}

#[test]
fn a_count_kitsu_or_anilist_does_not_give_keeps_the_listing() {
    let (rows, tail) = noragami();
    for t in [
        EntryTail {
            count: Some(13),
            ..tail.clone()
        },
        EntryTail {
            count: None,
            ..tail.clone()
        },
        EntryTail {
            anilist_count: Some(13),
            ..tail.clone()
        },
        EntryTail {
            anilist_count: None,
            ..tail.clone()
        },
    ] {
        assert_eq!(appended_extras(&rows, 12, &t), None, "{t:?}");
    }
}

#[test]
fn a_kitsu_tail_that_is_not_complete_and_dated_keeps_the_listing() {
    let (rows, tail) = noragami();
    let without = |n: u32| EntryTail {
        episodes: tail
            .episodes
            .iter()
            .filter(|e| e.number != n)
            .cloned()
            .collect(),
        ..tail.clone()
    };
    assert_eq!(appended_extras(&rows, 12, &without(12)), None, "no 12");
    assert_eq!(appended_extras(&rows, 12, &without(10)), None, "no 10");
    let mut undated = tail.clone();
    undated.episodes.iter_mut().for_each(|e| {
        if e.number == 11 {
            e.airdate = None;
        }
    });
    assert_eq!(appended_extras(&rows, 12, &undated), None, "undated");
    let mut past = tail.clone();
    past.episodes.push(TailEpisode {
        number: 13,
        title: Some("Thirteen".into()),
        airdate: Some("2014-03-30".into()),
    });
    assert_eq!(appended_extras(&rows, 12, &past), None, "a row past it");
    let unread = EntryTail {
        read_past_count: false,
        ..tail.clone()
    };
    assert_eq!(
        appended_extras(&rows, 12, &unread),
        None,
        "not read past it"
    );
}

/// The listing's own titles must be Kitsu's up to the count: a
/// listing that names its episodes differently, or numbers them one
/// off, proves nothing about its last rows.
#[test]
fn titles_that_do_not_line_up_with_kitsu_s_keep_the_listing() {
    let (rows, tail) = noragami();
    for slot in [10usize, 11, 12] {
        let mut renamed = rows.clone();
        renamed[slot - 1].title = Some("Another Name".into());
        assert_eq!(appended_extras(&renamed, 12, &tail), None, "slot {slot}");
        let mut untitled = rows.clone();
        untitled[slot - 1].title = None;
        assert_eq!(appended_extras(&untitled, 12, &tail), None, "slot {slot}");
    }
    let mut kitsu_untitled = tail.clone();
    kitsu_untitled.episodes.iter_mut().for_each(|e| {
        if e.number == 12 {
            e.title = None;
        }
    });
    assert_eq!(appended_extras(&rows, 12, &kitsu_untitled), None);
    // Shifted one: the provider's 12 is Kitsu's 11.
    let mut shifted = rows.clone();
    for i in 9..12 {
        shifted[i].title = rows[i - 1].title.clone();
    }
    assert_eq!(appended_extras(&shifted, 12, &tail), None, "shifted");
}

/// Case, spacing and punctuation are not a different title.
#[test]
fn titles_line_up_through_case_and_punctuation() {
    let (mut rows, tail) = noragami();
    rows[11].title = Some("  scrap OF a memory!! ".into());
    assert!(appended_extras(&rows, 12, &tail).is_some());
}

/// An appended row is read as an extra only by a name of its own: a
/// row with none, one that is only its number, or one Kitsu gives one
/// of its episodes is no evidence.
#[test]
fn an_appended_row_without_a_name_of_its_own_keeps_the_listing() {
    let (rows, tail) = noragami();
    for title in [
        None,
        Some("Episode 13"),
        Some("EP 13"),
        Some("Scrap of a Memory"),
    ] {
        let mut changed = rows.clone();
        changed[12].title = title.map(str::to_string);
        assert_eq!(appended_extras(&changed, 12, &tail), None, "{title:?}");
    }
}

#[test]
fn a_count_of_one_reads_its_one_episode() {
    let rows = plain(&["The Only One", "Bonus Story"]);
    let tail = EntryTail {
        finished: true,
        count: Some(1),
        anilist_count: Some(1),
        episodes: vec![TailEpisode {
            number: 1,
            title: Some("The only one".into()),
            airdate: Some("2020-01-01".into()),
        }],
        read_past_count: true,
        related: vec![Related {
            role: "side_story".into(),
            anime: true,
            subtype: Some("special".into()),
            status: Some("finished".into()),
            episode_count: Some(1),
            first_episodes: None,
        }],
        related_complete: true,
    };
    let fitted = appended_extras(&rows, 1, &tail).expect("appended");
    assert_eq!(tags(&fitted), vec![None, Some("1.1")]);
}

// ── where nothing may move ─────────────────────────────────────────

/// One of Kitsu's episode rows as the corpus records it: number,
/// canonical title, air date.
type KitsuRow = (Option<u32>, Option<String>, Option<String>);

/// One entry Kitsu relates a show to, as the corpus records it: role,
/// type, id, subtype, status, episode count, and a TV sequel's first
/// page of episode titles where it was read, in Kitsu's order.
type RelatedRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<u32>,
    Option<Vec<Option<String>>>,
);

#[derive(serde::Deserialize)]
struct Corpus {
    shows: Vec<CorpusShow>,
}

#[derive(serde::Deserialize, Clone)]
struct CorpusShow {
    kitsu_id: String,
    title: String,
    status: Option<String>,
    episode_count: Option<u32>,
    anilist: Option<CorpusAnilist>,
    kitsu_pages: std::collections::BTreeMap<String, Vec<KitsuRow>>,
    related: Vec<RelatedRow>,
    related_complete: bool,
    provider: CorpusProvider,
    appended: bool,
}

#[derive(serde::Deserialize, Clone)]
struct CorpusAnilist {
    status: Option<String>,
    episodes: Option<u32>,
    next_episode: Option<u32>,
}

#[derive(serde::Deserialize, Clone)]
struct CorpusProvider {
    rows: Vec<(u32, String, Option<String>)>,
}

fn corpus() -> Vec<CorpusShow> {
    let corpus: Corpus = serde_json::from_slice(&fixture("corpus.json")).expect("corpus parses");
    corpus.shows
}

fn corpus_show(kitsu: &str) -> CorpusShow {
    corpus()
        .into_iter()
        .find(|s| s.kitsu_id == kitsu)
        .unwrap_or_else(|| panic!("{kitsu} is recorded"))
}

impl CorpusShow {
    /// The listing as the reader returns it.
    fn listing(&self) -> Vec<EpisodeRef> {
        self.provider
            .rows
            .iter()
            .map(|(slot, number, title)| EpisodeRef {
                id: u64::from(*slot),
                number: *slot,
                number2: (*number != slot.to_string()).then(|| number.clone()),
                title: title.clone(),
            })
            .collect()
    }

    /// The tail production reads from these answers: the recorded
    /// Kitsu rows and AniList answer through the real parsers — which
    /// drop Kitsu's placeholder rows — and the evidence reader's own
    /// mapping, with the recorded relations.
    fn tail(&self) -> EntryTail {
        let data: Vec<serde_json::Value> = self
            .kitsu_pages
            .values()
            .flatten()
            .zip(0u32..)
            .map(|((number, title, airdate), i)| {
                serde_json::json!({
                    "id": i.to_string(),
                    "type": "episodes",
                    "attributes": {"number": number, "canonicalTitle": title, "airdate": airdate}
                })
            })
            .collect();
        let page = crate::meta::kitsu::parse_episodes_response(
            serde_json::json!({ "data": data }).to_string().as_bytes(),
        )
        .expect("a page");
        let airing = self
            .anilist
            .as_ref()
            .and_then(|a| {
                let next = a
                    .next_episode
                    .map(|e| serde_json::json!({"episode": e, "airingAt": 4_000_000_000u64}));
                let body = serde_json::json!({"data": {"Media": {
                    "status": a.status, "episodes": a.episodes,
                    "nextAiringEpisode": next, "airingSchedule": {"nodes": []}
                }}});
                crate::meta::anilist_airing::parse_airing_response(body.to_string().as_bytes())
                    .expect("an answer")
            })
            .unwrap_or_default();
        let mut tail = crate::commands::kitsu_tail::entry_tail(
            self.status.as_deref(),
            self.episode_count,
            &airing,
            page,
        );
        tail.related = self
            .related
            .iter()
            .map(|(role, kind, _, subtype, status, count, titles)| Related {
                role: role.clone(),
                anime: kind == "anime",
                subtype: subtype.clone(),
                status: status.clone(),
                episode_count: *count,
                // The corpus records the page's rows in order, so a
                // row's position is its number; Kitsu's empty title is
                // no title.
                first_episodes: titles.as_ref().map(|t| {
                    t.iter()
                        .zip(1u32..)
                        .map(|(title, number)| SequelEpisode {
                            number,
                            title: title.clone().filter(|t| !t.trim().is_empty()),
                        })
                        .collect()
                }),
            })
            .collect();
        tail.related_complete = self.related_complete;
        tail
    }

    /// The show as hianime listed it while only `past` of the rows
    /// past Kitsu's count were up.
    fn listed_up_to(&self, past: usize) -> Vec<EpisodeRef> {
        let count = self.episode_count.expect("counted") as usize;
        self.listing().into_iter().take(count + past).collect()
    }
}

/// Every show recorded: the three whose listings append extras gain
/// exactly the rows past the count as extras and keep every whole
/// episode where it was; on every other show — finished, airing, long
/// running, uncounted, short — the listing is the one picked, row for
/// row.
#[test]
fn on_every_recorded_show_only_appended_rows_move() {
    let shows = corpus();
    assert!(shows.len() > 100);
    for show in &shows {
        let rows = show.listing();
        let fitted = appended_count(&rows, show.episode_count)
            .and_then(|count| appended_extras(&rows, count, &show.tail()));
        if !show.appended {
            assert_eq!(fitted, None, "{} ({}) moved", show.title, show.kitsu_id);
            continue;
        }
        let count = show.episode_count.expect("counted");
        let fitted = fitted.unwrap_or_else(|| panic!("{} kept its rows", show.title));
        assert_eq!(&fitted[..count as usize], &rows[..count as usize]);
        assert_eq!(
            extra_episode_tags(&fitted).len(),
            rows.len() - count as usize,
            "{}",
            show.title
        );
        assert_eq!(kitsu_episode_cap(&fitted), Some(count));
    }
}

// ── what the rows past the count are ───────────────────────────────

fn explaining() -> Related {
    Related {
        role: "side_story".into(),
        anime: true,
        subtype: Some("OVA".into()),
        status: Some("finished".into()),
        episode_count: Some(2),
        first_episodes: None,
    }
}

/// Noragami's two rows past twelve are explained by the OVA Kitsu
/// relates it to, two episodes long. Without an entry of that shape
/// and length nothing says what the rows are.
#[test]
fn rows_no_related_entry_explains_keep_the_listing() {
    let (rows, tail) = noragami();
    assert!(appended_extras(&rows, 12, &tail).is_some(), "as recorded");
    let without = |keep: &dyn Fn(&Related) -> bool| EntryTail {
        related: tail.related.iter().filter(|r| keep(r)).cloned().collect(),
        ..tail.clone()
    };
    assert_eq!(
        appended_extras(
            &rows,
            12,
            &without(&|r| r.subtype.as_deref() != Some("OVA"))
        ),
        None,
        "no OVA"
    );
    for other in [
        Related {
            episode_count: Some(3),
            ..explaining()
        },
        Related {
            subtype: Some("TV".into()),
            ..explaining()
        },
        Related {
            status: Some("current".into()),
            ..explaining()
        },
        Related {
            anime: false,
            ..explaining()
        },
    ] {
        let t = EntryTail {
            related: vec![other.clone()],
            ..tail.clone()
        };
        assert_eq!(appended_extras(&rows, 12, &t), None, "{other:?}");
    }
    let part = EntryTail {
        related_complete: false,
        ..tail.clone()
    };
    assert_eq!(
        appended_extras(&rows, 12, &part),
        None,
        "relations read in part"
    );
}

/// A sequel still airing may be what the rows past the count are.
#[test]
fn a_sequel_still_airing_keeps_the_listing() {
    let (rows, mut tail) = noragami();
    tail.related.push(Related {
        role: "sequel".into(),
        anime: true,
        subtype: Some("TV".into()),
        status: Some("current".into()),
        episode_count: None,
        first_episodes: Some(Vec::new()),
    });
    assert_eq!(appended_extras(&rows, 12, &tail), None);
}

/// A TV sequel's episodes are checked against the rows past the count:
/// one whose page was not read leaves nothing to check them against.
#[test]
fn a_tv_sequel_whose_episodes_were_not_read_keeps_the_listing() {
    let (rows, mut tail) = noragami();
    for r in &mut tail.related {
        if r.role == "sequel" {
            r.first_episodes = None;
        }
    }
    assert_eq!(appended_extras(&rows, 12, &tail), None);
}

/// hianime lists some entries with the next cour merged in: Attack on
/// Titan Season 3 with Part 2, Gintama.'s Silver Soul arc with its
/// second half, Haikyuu!! To the Top with its second cour. While that
/// cour aired, the listing ran a few rows past the entry's count, and
/// every other piece of evidence held — the entry finished on Kitsu
/// and AniList, its titles lining up. The rows are the sequel's
/// episodes, which Kitsu lists under the sequel, and never extras of
/// the entry; Haikyuu!! even has a two-episode OVA that a two-row
/// window would match.
#[test]
fn a_sequel_cour_merged_into_the_listing_is_never_its_extras() {
    for kitsu in ["13569", "14095", "42059"] {
        let show = corpus_show(kitsu);
        let count = show.episode_count.expect("counted");
        let mut airing_sequel = show.tail();
        for r in &mut airing_sequel.related {
            if r.role == "sequel" {
                r.status = Some("current".into());
            }
        }
        for past in 1..=MAX_APPENDED as usize {
            let rows = show.listed_up_to(past);
            assert_eq!(appended_count(&rows, Some(count)), Some(count));
            assert_eq!(
                appended_extras(&rows, count, &show.tail()),
                None,
                "{} with {past} past, the sequel finished",
                show.title
            );
            assert_eq!(
                appended_extras(&rows, count, &airing_sequel),
                None,
                "{} with {past} past, the sequel airing",
                show.title
            );
        }
    }
}

/// Titles that are only numbers line up whatever each row holds, so
/// they prove nothing about the listing's numbering.
#[test]
fn titles_that_are_only_numbers_prove_no_numbering() {
    let (mut rows, mut tail) = noragami();
    for n in 10..=12u32 {
        rows[n as usize - 1].title = Some(format!("Episode {n}"));
        for e in &mut tail.episodes {
            if e.number == n {
                e.title = Some(format!("Episode {n}"));
            }
        }
    }
    assert_eq!(appended_extras(&rows, 12, &tail), None);
}

// ── a reading the evidence once proved ─────────────────────────────

/// A source whose evidence is `tail`, keeping what it is told to.
#[derive(Debug)]
struct Kept {
    tail: Option<EntryTail>,
    kept: std::sync::Mutex<Option<Remembered>>,
}

#[async_trait::async_trait]
impl TailSource for Kept {
    async fn tail(&self, _count: u32) -> Option<EntryTail> {
        self.tail.clone()
    }
    fn remembered(&self) -> Option<Remembered> {
        self.kept.lock().expect("lock").clone()
    }
    fn remember(&self, reading: Option<&Remembered>) {
        *self.kept.lock().expect("lock") = reading.cloned();
    }
}

fn picked(rows: Vec<EpisodeRef>) -> PickedShow {
    PickedShow {
        hit: crate::scraper::provider::BrowseHit {
            slug: "noragami-916".into(),
            title: "Noragami".into(),
            kind: None,
            jname: None,
        },
        episodes: rows,
        stitched: false,
    }
}

/// The evidence is cached for hours, not for good: AniList's answer
/// for a finished show lasts three. A request whose evidence cannot
/// be read — AniList limiting, Kitsu down, the budget spent — reads
/// the listing as the evidence last proved it, so it stamps the same
/// cap and extras as the request before it rather than putting the
/// OADs back as episodes 13 and 14.
#[tokio::test]
async fn a_reading_once_proved_stands_while_the_evidence_cannot_be_read() {
    let (rows, tail) = noragami();
    let source = Kept {
        tail: Some(tail),
        kept: std::sync::Mutex::new(None),
    };
    let first = fit_appended(picked(rows.clone()), Some(12), Some(&source)).await;
    assert_eq!(extra_episode_tags(&first.episodes), vec!["12.1", "12.2"]);
    let unread = Kept {
        tail: None,
        kept: std::sync::Mutex::new(source.remembered()),
    };
    let later = fit_appended(picked(rows.clone()), Some(12), Some(&unread)).await;
    assert_eq!(later.episodes, first.episodes);
    // Without a reading kept, no evidence is no reading.
    let never = Kept {
        tail: None,
        kept: std::sync::Mutex::new(None),
    };
    let plain = fit_appended(picked(rows.clone()), Some(12), Some(&never)).await;
    assert_eq!(plain.episodes, rows);
}

/// The kept reading is for the rows it was proved on: a listing whose
/// rows past the count are other rows, or another number of them, is
/// read without it.
#[tokio::test]
async fn a_kept_reading_is_for_the_rows_it_was_proved_on() {
    let (rows, _) = noragami();
    let kept = Remembered {
        slug: "noragami-916".into(),
        count: 12,
        ids: rows[12..].iter().map(|e| e.id).collect(),
    };
    let unread = |r: Remembered| Kept {
        tail: None,
        kept: std::sync::Mutex::new(Some(r)),
    };
    let mut other = rows.clone();
    other[13].id += 1;
    let got = fit_appended(picked(other.clone()), Some(12), Some(&unread(kept.clone()))).await;
    assert_eq!(got.episodes, other, "another row");
    let shorter: Vec<EpisodeRef> = rows[..13].to_vec();
    let got = fit_appended(
        picked(shorter.clone()),
        Some(12),
        Some(&unread(kept.clone())),
    )
    .await;
    assert_eq!(got.episodes, shorter, "fewer rows");
    let got = fit_appended(
        picked(rows.clone()),
        Some(12),
        Some(&unread(Remembered { count: 11, ..kept })),
    )
    .await;
    assert_eq!(got.episodes, rows, "another count");
}

/// Evidence that reads and no longer proves the reading takes the
/// kept one away: the next request without evidence reads the listing
/// as picked.
#[tokio::test]
async fn evidence_that_no_longer_proves_the_reading_forgets_it() {
    let (rows, mut tail) = noragami();
    tail.finished = false;
    let (_, proved) = noragami();
    let source = Kept {
        tail: Some(proved),
        kept: std::sync::Mutex::new(None),
    };
    fit_appended(picked(rows.clone()), Some(12), Some(&source)).await;
    assert!(source.remembered().is_some(), "kept once proved");
    let disproved = Kept {
        tail: Some(tail),
        kept: std::sync::Mutex::new(source.remembered()),
    };
    let got = fit_appended(picked(rows.clone()), Some(12), Some(&disproved)).await;
    assert_eq!(got.episodes, rows);
    assert_eq!(disproved.remembered(), None);
}

// ── history rows an earlier version wrote ──────────────────────────

fn noragami_reading() -> Remembered {
    let (rows, _) = noragami();
    Remembered {
        slug: "noragami-916".into(),
        count: 12,
        ids: rows[12..].iter().map(|e| e.id).collect(),
    }
}

/// An earlier version recorded Noragami's OADs as episodes 13 and 14:
/// under the kept reading those rows read back as the extras they are.
#[test]
fn a_slot_the_reading_tagged_reads_back_as_its_extra() {
    let reading = noragami_reading();
    let key = "hianime:noragami-916";
    assert_eq!(extra_for_slot(&reading, key, "13"), Some("12.1".into()));
    assert_eq!(extra_for_slot(&reading, key, " 14 "), Some("12.2".into()));
}

/// Anything else is read as it was written: an episode up to the
/// count, a slot past the rows the reading tagged, an extra already,
/// a range, another show.
#[test]
fn every_other_row_reads_back_as_written() {
    let reading = noragami_reading();
    let key = "hianime:noragami-916";
    for ep in ["1", "12", "15", "12.1", "1-12", "", "x"] {
        assert_eq!(extra_for_slot(&reading, key, ep), None, "{ep:?}");
    }
    assert_eq!(extra_for_slot(&reading, "hianime:other-show-1", "13"), None);
    assert_eq!(
        extra_for_slot(&reading, "noragami-916", "13"),
        None,
        "a bare id"
    );
}

// ── sequels announced, and sequels Kitsu has not titled ───────────

/// Kitsu adds a sequel long before it airs. One only announced has no
/// episodes the listing could hold, so it neither refuses the reading
/// nor needs a page of titles.
#[test]
fn a_sequel_only_announced_keeps_the_reading() {
    for status in ["upcoming", "tba", "unreleased"] {
        let (rows, mut tail) = noragami();
        tail.related.push(Related {
            role: "sequel".into(),
            anime: true,
            subtype: Some("TV".into()),
            status: Some(status.into()),
            episode_count: None,
            first_episodes: Some(Vec::new()),
        });
        assert!(appended_extras(&rows, 12, &tail).is_some(), "{status}");
    }
}

/// A TV sequel that has aired, but whose first page Kitsu has not
/// titled, leaves nothing to tell its episodes from extras: Haikyuu!!
/// To the Top's second cour, two rows past the count, with the entry's
/// own last episodes titled as hianime titles them and its two-episode
/// OVA there to match the window.
#[test]
fn a_sequel_kitsu_has_not_titled_keeps_the_listing() {
    let show = corpus_show("42059");
    let count = show.episode_count.expect("counted");
    let rows = show.listed_up_to(2);
    let mut tail = show.tail();
    tail.episodes = (1..=count)
        .map(|n| TailEpisode {
            number: n,
            title: rows[n as usize - 1].title.clone(),
            airdate: Some("2020-01-01".into()),
        })
        .collect();
    for r in &mut tail.related {
        if r.role == "sequel" {
            r.first_episodes = Some(Vec::new());
        }
    }
    assert_eq!(appended_extras(&rows, count, &tail), None);
}

/// A kept reading is for the show it was proved on: another listing's
/// rows, whatever their ids, are read without it.
#[tokio::test]
async fn a_kept_reading_is_for_the_show_it_was_proved_on() {
    let (rows, _) = noragami();
    let source = Kept {
        tail: None,
        kept: std::sync::Mutex::new(Some(Remembered {
            slug: "another-show-1".into(),
            ..noragami_reading()
        })),
    };
    let got = fit_appended(picked(rows.clone()), Some(12), Some(&source)).await;
    assert_eq!(got.episodes, rows);
}

/// A listing of the same show that no longer runs past the count — a
/// count Kitsu raised to take the rows in — forgets the reading; one
/// of another show leaves it.
#[tokio::test]
async fn a_listing_no_longer_past_the_count_forgets_the_reading() {
    let (rows, _) = noragami();
    let source = Kept {
        tail: None,
        kept: std::sync::Mutex::new(Some(noragami_reading())),
    };
    let mut other = picked(rows.clone());
    other.hit.slug = "another-show-1".into();
    fit_appended(other, Some(14), Some(&source)).await;
    assert_eq!(
        source.remembered(),
        Some(noragami_reading()),
        "another show"
    );
    let got = fit_appended(picked(rows.clone()), Some(14), Some(&source)).await;
    assert_eq!(got.episodes, rows);
    assert_eq!(source.remembered(), None);
}

// ── a sequel's first episodes, by number ───────────────────────────

/// Slime's first season (Kitsu 41024, twenty-four episodes) with the
/// first `k` rows of its TV sequel's hianime listing appended, as
/// hianime lists a merged cour.
fn slime_with_sequel_rows(k: usize) -> (Vec<EpisodeRef>, EntryTail) {
    let show = corpus_show("41024");
    let mut rows = show.listing();
    let count = rows.len();
    let sequel = listing(487);
    for (i, row) in sequel.into_iter().take(k).enumerate() {
        rows.push(EpisodeRef {
            number: u32::try_from(count + i + 1).expect("a slot"),
            ..row
        });
    }
    (rows, show.tail())
}

/// The sequel's page in the shape Kitsu gives many a second season:
/// its first `untitled` episodes dated but not titled, the rest titled.
fn sequel_page(untitled: u32, titles: &[&str]) -> Vec<SequelEpisode> {
    (1..=12u32)
        .map(|n| SequelEpisode {
            number: n,
            title: (n > untitled).then(|| {
                titles
                    .get(n as usize - 1)
                    .map_or_else(|| format!("Sequel {n}"), |t| (*t).to_string())
            }),
        })
        .collect()
}

fn set_sequel_page(tail: &mut EntryTail, page: Vec<SequelEpisode>) {
    for r in &mut tail.related {
        if r.role == "sequel" && r.subtype.as_deref() == Some("TV") {
            r.first_episodes = Some(page.clone());
        }
    }
}

/// A TV sequel whose first episodes Kitsu has dated but not titled,
/// while it has titled later ones, leaves nothing to tell its first
/// episodes from extras — however many titles the page carries past
/// them. Slime's first season relates a one-episode special, a
/// two-episode ONA, a three-episode OVA and a five-episode OVA, so each
/// of those windows has an entry that would explain it; with its
/// second season's first rows appended and Kitsu titling that season
/// from episode 3 on, the rows must stay the sequel's episodes. The
/// season's other TV relation, The Slime Diaries, is given a titled
/// page throughout, so the sequel's page alone varies.
#[test]
fn a_sequel_whose_first_episodes_kitsu_has_not_titled_keeps_the_listing() {
    for k in [1usize, 2, 3, 5] {
        let (rows, mut tail) = slime_with_sequel_rows(k);
        let count = 24;
        assert_eq!(appended_count(&rows, Some(count)), Some(count));
        set_tv_pages(&mut tail, Some(sequel_page(0, &[])));
        set_sequel_page(&mut tail, sequel_page(u32::try_from(k).expect("k"), &[]));
        assert_eq!(
            appended_extras(&rows, count, &tail),
            None,
            "{k} rows, untitled"
        );
        // The hole is what refused: the same page with the first
        // episodes titled as hianime titles them refuses on the names,
        // and titled otherwise reads the rows as the explaining entry.
        let hianime: Vec<&str> = rows[24..]
            .iter()
            .filter_map(|r| r.title.as_deref())
            .collect();
        set_sequel_page(&mut tail, sequel_page(0, &hianime));
        assert_eq!(
            appended_extras(&rows, count, &tail),
            None,
            "{k} rows, the sequel's names"
        );
        set_sequel_page(&mut tail, sequel_page(0, &[]));
        assert!(
            appended_extras(&rows, count, &tail).is_some(),
            "{k} rows, other names"
        );
    }
}

/// The shapes a sequel's page can take, over Noragami's two rows past
/// twelve: a first episode missing from the page (a placeholder Kitsu
/// listed with neither title nor date, which the reader drops) or
/// titled with nothing but spaces refuses; a title missing past the
/// rows under question does not matter, nor does the page's order; and
/// every aired TV sequel has to pass, not one of them.
#[test]
fn a_sequel_page_is_read_by_episode_number() {
    let (rows, proved) = noragami();
    let aragoto = |page: Vec<SequelEpisode>| {
        let mut tail = proved.clone();
        set_sequel_page(&mut tail, page);
        tail
    };
    let titled = |n: u32| SequelEpisode {
        number: n,
        title: Some(format!("Aragoto {n}")),
    };
    let untitled = |n: u32| SequelEpisode {
        number: n,
        title: None,
    };
    let blank = |n: u32| SequelEpisode {
        number: n,
        title: Some("   ".into()),
    };
    let full: Vec<SequelEpisode> = (1..=13).map(titled).collect();
    assert!(appended_extras(&rows, 12, &aragoto(full.clone())).is_some());
    let missing_first: Vec<SequelEpisode> = (2..=13).map(titled).collect();
    assert_eq!(
        appended_extras(&rows, 12, &aragoto(missing_first)),
        None,
        "episode 1 missing"
    );
    let mut second_untitled = full.clone();
    second_untitled[1] = untitled(2);
    assert_eq!(
        appended_extras(&rows, 12, &aragoto(second_untitled)),
        None,
        "episode 2 untitled"
    );
    let mut second_blank = full.clone();
    second_blank[1] = blank(2);
    assert_eq!(
        appended_extras(&rows, 12, &aragoto(second_blank)),
        None,
        "episode 2 blank"
    );
    let mut later_untitled = full.clone();
    for e in &mut later_untitled[2..] {
        e.title = None;
    }
    assert!(
        appended_extras(&rows, 12, &aragoto(later_untitled)).is_some(),
        "past the rows"
    );
    let mut out_of_order = full.clone();
    out_of_order.reverse();
    assert!(
        appended_extras(&rows, 12, &aragoto(out_of_order)).is_some(),
        "order is nothing"
    );
    // A second aired TV sequel with its first episode untitled refuses
    // even beside one fully titled.
    let mut two = aragoto(full);
    two.related.push(Related {
        role: "sequel".into(),
        anime: true,
        subtype: Some("TV".into()),
        status: Some("finished".into()),
        episode_count: Some(12),
        first_episodes: Some(vec![untitled(1), titled(2), titled(3)]),
    });
    assert_eq!(appended_extras(&rows, 12, &two), None, "a second sequel");
}

// ── TV entries related in any role ─────────────────────────────────

/// The TV entries Kitsu relates `tail`'s show to, given `page` as their
/// first episodes, whatever their role.
fn set_tv_pages(tail: &mut EntryTail, page: Option<Vec<SequelEpisode>>) {
    for r in &mut tail.related {
        if r.anime && r.subtype.as_deref() == Some("TV") {
            r.first_episodes = page.clone();
        }
    }
}

/// Kitsu relates Slime's first season to The Slime Diaries, a TV entry,
/// as `other`; its Kitsu page is twelve placeholder rows. With the
/// Diaries' first rows appended to the season's listing, as hianime
/// lists a merged cour, the special, ONA and OVAs the season relates
/// would explain one, two, three or five rows, and nothing named the
/// rows the Diaries' episodes — a TV entry's first episodes are
/// checked whatever Kitsu calls the relation. The season's TV sequel is
/// given a titled page throughout (Kitsu titles none of it, and
/// untitled it would refuse on its own), so the Diaries' page alone
/// decides: as recorded, titling none of its episodes, it refuses;
/// titled as hianime titles the rows, the names refuse; titled
/// otherwise, the rows read as the explaining entry's.
#[test]
fn a_tv_entry_related_in_any_role_has_its_first_episodes_checked() {
    let show = corpus_show("41024");
    let diaries = listing(1574);
    for k in [1usize, 2, 3, 5] {
        let mut rows = show.listing();
        let count = u32::try_from(rows.len()).expect("a count");
        for (i, row) in diaries.iter().take(k).enumerate() {
            rows.push(EpisodeRef {
                number: count + u32::try_from(i + 1).expect("a slot"),
                ..row.clone()
            });
        }
        let mut tail = show.tail();
        set_sequel_page(&mut tail, sequel_page(0, &[]));
        let other = tail
            .related
            .iter()
            .find(|r| r.role == "other" && r.subtype.as_deref() == Some("TV"))
            .expect("the Diaries");
        assert!(
            other
                .first_episodes
                .as_ref()
                .is_some_and(|p| p.iter().all(|e| e.title.is_none())),
            "the Diaries' page as recorded titles no episode: {:?}",
            other.first_episodes
        );
        assert_eq!(
            appended_extras(&rows, count, &tail),
            None,
            "{k} rows, as recorded"
        );
        let hianime: Vec<&str> = rows[24..]
            .iter()
            .filter_map(|r| r.title.as_deref())
            .collect();
        let mut named = tail.clone();
        set_tv_pages(&mut named, Some(sequel_page(0, &hianime)));
        assert_eq!(
            appended_extras(&rows, count, &named),
            None,
            "{k} rows, hianime's names"
        );
        let mut other_names = tail.clone();
        set_tv_pages(&mut other_names, Some(sequel_page(0, &[])));
        assert!(
            appended_extras(&rows, count, &other_names).is_some(),
            "{k} rows, other names"
        );
    }
}

/// Noragami Aragoto's one TV relation is its prequel. Its OADs read as
/// extras with the prequel's page as recorded; the same rows titled as
/// the prequel's first two episodes do not, and a prequel page Kitsu
/// had not titled refuses as a sequel's does.
#[test]
fn a_prequel_s_first_episodes_are_checked_too() {
    let rows = listing(913);
    let tail = recorded_tail(10800);
    let prequel = tail
        .related
        .iter()
        .find(|r| r.role == "prequel")
        .expect("Noragami");
    assert_eq!(prequel.subtype.as_deref(), Some("TV"));
    assert_eq!(
        prequel
            .first_episodes
            .as_ref()
            .map(|p| p.iter().filter(|e| e.title.is_some()).count()),
        Some(12)
    );
    assert!(appended_extras(&rows, 13, &tail).is_some(), "as recorded");
    let mut as_prequel = rows.clone();
    for (row, n) in as_prequel[13..].iter_mut().zip(1u32..) {
        row.title = prequel
            .first_episodes
            .as_ref()
            .and_then(|p| p.iter().find(|e| e.number == n))
            .and_then(|e| e.title.clone());
    }
    assert_eq!(
        appended_extras(&as_prequel, 13, &tail),
        None,
        "the prequel's names"
    );
    let mut untitled = tail.clone();
    set_tv_pages(&mut untitled, Some(Vec::new()));
    assert_eq!(
        appended_extras(&rows, 13, &untitled),
        None,
        "the prequel untitled"
    );
}

/// An entry airing now may be what the rows past the count are,
/// whatever Kitsu calls the relation and whatever its format: Stone
/// Ocean Part 3's sequel is an ONA Kitsu calls current, and a non-TV
/// entry's page is never read, so nothing but its airing tells its
/// cour from extras. A music video is no cour.
#[test]
fn an_entry_airing_now_keeps_the_listing_whatever_its_format() {
    for (role, subtype) in [
        ("sequel", "ONA"),
        ("side_story", "OVA"),
        ("other", "special"),
        ("sequel", "movie"),
    ] {
        let (rows, mut tail) = noragami();
        tail.related.push(Related {
            role: role.into(),
            anime: true,
            subtype: Some(subtype.into()),
            status: Some("current".into()),
            episode_count: None,
            first_episodes: None,
        });
        assert_eq!(appended_extras(&rows, 12, &tail), None, "{role} {subtype}");
    }
    let (rows, mut tail) = noragami();
    tail.related.push(Related {
        role: "other".into(),
        anime: true,
        subtype: Some("music".into()),
        status: Some("current".into()),
        episode_count: Some(1),
        first_episodes: None,
    });
    assert!(appended_extras(&rows, 12, &tail).is_some(), "a music video");
}

/// A TV entry airing now may be what the rows past the count are,
/// whatever Kitsu calls the relation.
#[test]
fn a_tv_entry_airing_now_keeps_the_listing_whatever_its_role() {
    for role in ["other", "side_story", "prequel", "spin_off"] {
        let (rows, mut tail) = noragami();
        tail.related.push(Related {
            role: role.into(),
            anime: true,
            subtype: Some("TV".into()),
            status: Some("current".into()),
            episode_count: None,
            first_episodes: Some(
                (1..=3)
                    .map(|n| SequelEpisode {
                        number: n,
                        title: Some(format!("Other {n}")),
                    })
                    .collect(),
            ),
        });
        assert_eq!(appended_extras(&rows, 12, &tail), None, "{role}");
    }
}

/// The entry that explains the rows is what the rows are, so its own
/// episodes' titles are never held against them: Kitsu titles
/// Satsuriku no Tenshi's ONA as hianime titles the web episodes it
/// appends, case aside, and Noragami's OVA as hianime titles the OADs
/// past their "OAD 01: ". Given those pages, the readings stand — a
/// name check on a non-TV relation would refuse Satsuriku.
#[test]
fn the_explaining_entry_s_own_titles_are_not_held_against_the_rows() {
    let page = |id: u32| -> Vec<SequelEpisode> {
        crate::meta::kitsu::parse_episodes_response(&fixture(&format!(
            "kitsu-episodes-{id}-p1.json"
        )))
        .expect("recorded page")
        .into_iter()
        .filter_map(|e| {
            Some(SequelEpisode {
                number: e.number?,
                title: e.canonical_title,
            })
        })
        .collect()
    };
    let ona = page(41963);
    let rows = listing(1549);
    assert!(
        ona.iter().zip(&rows[12..]).all(|(e, row)| same_title(
            e.title.as_deref().expect("titled"),
            row.title.as_deref().expect("titled")
        )),
        "Kitsu's ONA titles are hianime's rows': {ona:?}"
    );
    let mut tail = recorded_tail(13715);
    for r in &mut tail.related {
        if r.subtype.as_deref() == Some("ONA") {
            r.first_episodes = Some(ona.clone());
        }
    }
    assert!(appended_extras(&rows, 12, &tail).is_some());
    let (rows, mut tail) = noragami();
    for r in &mut tail.related {
        if r.subtype.as_deref() == Some("OVA") {
            r.first_episodes = Some(page(8152));
        }
    }
    assert!(appended_extras(&rows, 12, &tail).is_some());
}

// ── stitched listings ──────────────────────────────────────────────

/// A listing the split chain stitches from parts numbered from 1 comes
/// out untagged and numbered by position, exactly as a single listing
/// does — Steel Ball Run's premiere and its 2nd Stage stitch into rows
/// 1, 2, 3, … — so its shape says nothing about how it was made. The
/// chain stitches parts whose sum fits the count within the tolerance,
/// so the stitched listing can run a row or two past the count, and
/// those rows are the later part's own episodes, which Kitsu has not
/// counted yet. They are never read as extras, whatever the evidence:
/// here Noragami's listing as a one-episode premiere and a thirteen-row
/// second part, with Noragami's recorded evidence and its two-episode
/// OVA there to explain two rows.
#[tokio::test]
async fn a_stitched_listing_is_never_read_for_appended_extras() {
    let (rows, tail) = noragami();
    let renumbered = |part: &[EpisodeRef]| -> Vec<EpisodeRef> {
        part.iter()
            .zip(1u32..)
            .map(|(e, n)| EpisodeRef {
                number: n,
                ..e.clone()
            })
            .collect()
    };
    let premiere = renumbered(&rows[..1]);
    let second = renumbered(&rows[1..]);
    let merged =
        crate::commands::play_native_merge::merge_parts(&[premiere.as_slice(), second.as_slice()]);
    assert!(merged.iter().all(|e| e.number2.is_none()), "{merged:?}");
    assert_eq!(
        appended_count(&merged, Some(12)),
        Some(12),
        "the shape asks"
    );
    assert!(
        appended_extras(&merged, 12, &tail).is_some(),
        "the evidence alone would read them"
    );
    let source = Kept {
        tail: Some(tail),
        kept: std::sync::Mutex::new(None),
    };
    let stitched = PickedShow {
        stitched: true,
        ..picked(merged.clone())
    };
    let got = fit_appended(stitched, Some(12), Some(&source)).await;
    assert_eq!(got.episodes, merged);
    assert_eq!(source.remembered(), None, "nothing kept for it");
}
