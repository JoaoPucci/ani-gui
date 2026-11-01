//! The cross-cour verdicts against Kitsu entries as Kitsu serves them
//! (slug, canonical title, titles and abbreviations read from
//! kitsu.io on 2026-11-01), each paired with the hianime card title the
//! picker's recorded pools list for it.

use super::*;
use crate::commands::cour::cour_from_title;
use std::collections::HashMap;

fn entry(
    id: &str,
    slug: &str,
    canonical: &str,
    titles: &[(&str, &str)],
    abbreviated: &[&str],
) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: id.into(),
        canonical_title: canonical.into(),
        titles: titles
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<HashMap<_, _>>(),
        abbreviated_titles: abbreviated.iter().map(|s| (*s).to_string()).collect(),
        slug: Some(slug.into()),
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: None,
        average_rating: None,
        subtype: Some("TV".into()),
        status: Some("finished".into()),
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        anilist_banner: None,
        cover_image: None,
    }
}

fn kitsu(id: &str) -> KitsuAnimeRef {
    match id {
        "5971" => entry(
            "5971",
            "gintama-2",
            "Gintama'",
            &[
                ("en_jp", "Gintama'"),
                ("en_us", "Gintama Season 2"),
                ("ja_jp", "銀魂'"),
            ],
            &["Gintama (2011)"],
        ),
        "10083" => entry(
            "10083",
            "gintama-2015-tv",
            "Gintama°",
            &[
                ("en_jp", "Gintama°"),
                ("en_us", "Gintama Season 4"),
                ("ja_jp", "銀魂°"),
            ],
            &["Gintama' (2015)"],
        ),
        "12553" => entry(
            "12553",
            "gintama-2017",
            "Gintama.",
            &[
                ("en_jp", "Gintama."),
                ("en_us", "Gintama Season 5"),
                ("ja_jp", "銀魂。"),
            ],
            &["Gintama (2017)"],
        ),
        "8671" => entry(
            "8671",
            "shingeki-no-kyojin-2",
            "Attack on Titan Season 2",
            &[
                ("en", "Attack on Titan Season 2"),
                ("en_jp", "Shingeki no Kyojin Season 2"),
                ("ja_jp", "進撃の巨人 Season 2"),
            ],
            &[],
        ),
        "13569" => entry(
            "13569",
            "shingeki-no-kyojin-3",
            "Attack on Titan Season 3",
            &[
                ("en", "Attack on Titan Season 3"),
                ("en_jp", "Shingeki no Kyojin Season 3"),
                ("ja_jp", "進撃の巨人 Season 3"),
            ],
            &[],
        ),
        "45556" => entry(
            "45556",
            "golden-kamuy-4",
            "Golden Kamuy 4th Season",
            &[
                ("en", "Golden Kamuy Season 4"),
                ("en_jp", "Golden Kamuy 4th Season"),
                ("ja_jp", "ゴールデンカムイ 第四期"),
            ],
            &[],
        ),
        "42938" => entry(
            "42938",
            "haikyuu-to-the-top-2",
            "Haikyuu!! TO THE TOP 2",
            &[
                ("en", "Haikyuu!!: To the Top Part 2"),
                ("en_jp", "Haikyuu!! TO THE TOP 2"),
                ("ja_jp", "ハイキュー!! TO THE TOP 2"),
            ],
            &[],
        ),
        "45857" => entry(
            "45857",
            "jujutsu-kaisen-2",
            "Jujutsu Kaisen Season 2",
            &[
                ("en", "Jujutsu Kaisen Season 2"),
                ("en_jp", "Jujutsu Kaisen Season 2"),
                ("ja_jp", "呪術廻戦 第2期"),
            ],
            &[
                "JJK2",
                "Jujutsu Kaisen 2nd Season",
                "Jujutsu Kaisen: Kaigyoku Gyokusetsu / Shibuya Jihen",
                "Jujutsu Kaisen: Hidden Inventory / Premature Death",
                "Jujutsu Kaisen: Shibuya Incident",
            ],
        ),
        "42632" => entry(
            "42632",
            "kaguya-sama-wa-kokurasetai-tensai-tachi-no-renai-zunousen-2",
            "Kaguya-sama wa Kokurasetai?: Tensaitachi no Renai Zunousen",
            &[
                ("en", "Kaguya-sama: Love is War?"),
                (
                    "en_jp",
                    "Kaguya-sama wa Kokurasetai?: Tensaitachi no Renai Zunousen",
                ),
                ("ja_jp", "かぐや様は告らせたい？～天才たちの恋愛頭脳戦～"),
            ],
            &[
                "Kaguya-sama wa Kokurasetai: Tensai-tachi no Renai Zunousen 2",
                "Kaguya-sama: Love is War Season 2",
                "Kaguya-sama wa Kokurasetai?: Tensai-tachi no Renai Zunousen",
            ],
        ),
        "46561" => entry(
            "46561",
            "kidou-senshi-gundam-suisei-no-majo-2",
            "Kidou Senshi Gundam: Suisei no Majo Season 2",
            &[
                ("en", "Mobile Suit Gundam: The Witch from Mercury Season 2"),
                ("en_jp", "Kidou Senshi Gundam: Suisei no Majo Season 2"),
                ("ja_jp", "機動戦士ガンダム 水星の魔女 Season2"),
            ],
            &[
                "G-Witch 2",
                "Mobile Suit Gundam: The Witch from Mercury Cour 2",
                "Kidou Senshi Gundam: Suisei no Majo Part 2",
                "機動戦士ガンダム 水星の魔女 2クール",
            ],
        ),
        "42198" => entry(
            "42198",
            "re-zero-kara-hajimeru-isekai-seikatsu-2",
            "Re:Zero kara Hajimeru Isekai Seikatsu 2",
            &[
                ("en", "Re:ZERO -Starting Life in Another World- Season 2"),
                ("en_jp", "Re:Zero kara Hajimeru Isekai Seikatsu 2"),
                ("en_us", "Re:ZERO -Starting Life in Another World- Season 2"),
                ("ja_jp", "Re:ゼロから始める異世界生活２"),
            ],
            &[],
        ),
        "42196" => entry(
            "42196",
            "tensei-shitara-slime-datta-ken-2",
            "Tensei shitara Slime Datta Ken 2",
            &[
                ("en", "That Time I Got Reincarnated as a Slime 2"),
                ("en_jp", "Tensei shitara Slime Datta Ken 2"),
                ("ja_jp", "転生したらスライムだった件2"),
            ],
            &[
                "TenSura 2",
                "Tensei Shitara Slime Datta Ken 2nd Season Part 1",
            ],
        ),
        "8739" => entry(
            "8739",
            "jojo-s-bizarre-adventure-stardust-crusaders-2",
            "JoJo no Kimyou na Bouken: Stardust Crusaders 2nd Season",
            &[
                (
                    "en",
                    "JoJo's Bizarre Adventure: Stardust Crusaders - Battle in Egypt",
                ),
                (
                    "en_jp",
                    "JoJo no Kimyou na Bouken: Stardust Crusaders 2nd Season",
                ),
                ("ja_jp", "ジョジョの奇妙な冒険 スターダストクルセイダース"),
            ],
            &[
                "JoJo's Bizarre Adventure Part 3",
                "JoJo's Bizarre Adventure: Stardust Crusaders - Egypt Arc",
            ],
        ),
        // Stone Ocean Part 1 under the slug it carried when the guard
        // was written for it.
        "44294" => entry(
            "44294",
            "jojo-no-kimyou-na-bouken-stone-ocean",
            "JoJo's Bizarre Adventure: Stone Ocean",
            &[
                ("en", "JoJo's Bizarre Adventure: Stone Ocean"),
                ("en_jp", "JoJo no Kimyou na Bouken: Stone Ocean"),
                ("en_us", "JoJo's Bizarre Adventure STONE OCEAN"),
                ("ja_jp", "ジョジョの奇妙な冒険 ストーンオーシャン"),
            ],
            &["JoJo no Kimyou na Bouken: Sutōn Ōshan", "Jojo part 6"],
        ),
        other => panic!("no recorded entry {other}"),
    }
}

/// The mapping guard's verdict for a watch titled `title` paired with
/// entry `id`, as `cour_pairing_disagrees` reaches it.
fn refuses(title: &str, id: &str) -> bool {
    let cour = cour_from_title(title).expect("a title with cour evidence");
    entry_disagrees(title, cour, &kitsu(id))
}

/// The provider names these entries exactly as Kitsu titles them, and
/// their slugs mark the season some other way than a cour suffix: a
/// bare number (`gintama-2`), a year (`gintama-2015-tv`). Each is the
/// right entry for the card — the picker's recorded pick, or the
/// entry its own page played.
#[test]
fn an_entry_titled_as_the_provider_titles_it_is_not_refused() {
    for (title, id) in [
        ("Gintama Season 2", "5971"),
        ("Gintama Season 4", "10083"),
        ("Gintama Season 5", "12553"),
        ("Attack on Titan Season 2", "8671"),
        ("Attack on Titan Season 3", "13569"),
        ("Golden Kamuy Season 4", "45556"),
        ("Haikyuu!!: To the Top 2nd Season", "42938"),
        ("Jujutsu Kaisen 2nd Season", "45857"),
        ("Kaguya-sama: Love is War Season 2", "42632"),
        (
            "Mobile Suit Gundam: The Witch from Mercury Season 2",
            "46561",
        ),
        ("Re:ZERO -Starting Life in Another World- Season 2", "42198"),
    ] {
        assert!(!refuses(title, id), "{title} → {id}");
    }
}

/// The pairing the guard was written for: Stone Ocean Part 2's stream
/// recorded against Part 1's entry, whose slug carries no suffix.
#[test]
fn stone_ocean_part_two_against_part_one_stays_refused() {
    assert!(refuses(
        "JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2",
        "44294"
    ));
    assert!(refuses(
        "JoJo's Bizarre Adventure Part 6: Stone Ocean Part 2",
        "44294"
    ));
}

/// Wrong pairings whose cour number matches one the entry's titles
/// carry. Each card is another recorded entry's listing; reading the
/// season off the titles' number alone would accept all three.
#[test]
fn a_matching_number_under_another_name_stays_refused() {
    for (title, id) in [
        ("Attack on Titan Season 3 Part 2", "8671"),
        ("Attack on Titan: Final Season, Part 2", "8671"),
        ("JoJo's Bizarre Adventure: Stone Ocean Part 3", "8739"),
    ] {
        assert!(refuses(title, id), "{title} → {id}");
    }
}

/// The right entry for the card, still refused: Kitsu titles it "That
/// Time I Got Reincarnated as a Slime 2" and "2nd Season Part 1", never
/// "Season 2", so no title names it as the provider does, and its
/// slug's bare `-2` is not a cour suffix. Refused before this
/// comparison existed too; it does not regress.
#[test]
fn slime_season_two_is_still_refused() {
    assert!(refuses(
        "That Time I Got Reincarnated as a Slime Season 2",
        "42196"
    ));
}

/// The reverse resolve searches Kitsu with the slug's words and takes
/// the first hit that agrees with the cour they carry. A hit titled
/// as the words name it agrees.
#[test]
fn a_hit_titled_as_the_slug_words_name_it_is_taken() {
    for (term, id) in [
        ("gintama season 2", "5971"),
        ("gintama season 4", "10083"),
        ("gintama season 5", "12553"),
        ("attack on titan season 2", "8671"),
        ("jujutsu kaisen season 2", "45857"),
    ] {
        let cour = crate::commands::cour::cour_from_slug(&term.replace(' ', "-"));
        assert!(hit_agrees(cour, term, &kitsu(id)), "{term} → {id}");
    }
}

/// A hit whose slug disagrees and whose titles do not name the term
/// is still passed over.
#[test]
fn a_hit_of_another_cour_is_still_passed_over() {
    assert!(!hit_agrees(
        Some(2),
        "jojos bizarre adventure stone ocean part 2",
        &kitsu("44294")
    ));
    assert!(!hit_agrees(
        Some(2),
        "attack on titan season 3 part 2",
        &kitsu("8671")
    ));
}
