#!/usr/bin/env python3
"""The oracle's own tests: that it models docs/title-resolution.md, that
its generated set is reproducible, and that its comparison refuses what
it must. Standard library only — CI runs this with the runner's python3.

    python3 -I -m unittest discover -s tests/picker-oracle -p 'test_*.py'
"""

import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import oracle  # noqa: E402

E = 12


def pool(titles, count, rows, year=2020, dead=(), **names):
    """A pool; `names` may give the cards' romanized names (`jnames`) and
    the pages' names (`page_names`), as a case does."""
    return {
        "id": "t",
        "titles": list(titles),
        "count": count,
        "entry_year": year,
        "rows": [list(r) for r in rows],
        "dead": list(dead),
        "family": "test",
        **names,
    }


def pick(slug, count=E):
    return {"pick": slug, "count": count}


class Generation(unittest.TestCase):
    def test_the_set_is_the_same_every_time(self):
        self.assertEqual(oracle.gen(), oracle.gen())

    def test_every_id_is_unique(self):
        ids = [c["id"] for c in oracle.gen()]
        self.assertEqual(len(ids), len(set(ids)))

    def test_a_generated_id_follows_the_pool_not_its_position(self):
        case = next(c for c in oracle.gen() if c["family"].startswith("admission") and len(c["rows"]) > 1)
        again = oracle.case_id(
            case["family"], case["titles"], case["count"], case["entry_year"], case["rows"], case.get("dead", [])
        )
        self.assertEqual(again, case["id"])
        moved = oracle.case_id(
            case["family"], case["titles"], case["count"], case["entry_year"], list(reversed(case["rows"])), []
        )
        self.assertNotEqual(moved, case["id"], "the provider's order is part of the pool")

    def test_the_hand_written_sets_ride_along_with_their_expectations(self):
        by_id = {c["id"]: c for c in oracle.gen()}
        total = 0
        for name in sorted(os.listdir(os.path.join(HERE, "cases"))):
            with open(os.path.join(HERE, "cases", name), encoding="utf-8") as fh:
                for written in json.load(fh):
                    total += 1
                    self.assertIn(written["id"], by_id, name)
                    self.assertEqual(by_id[written["id"]]["expect"], written["expect"], written["id"])
        self.assertGreater(total, 100)

    def test_every_case_carries_an_expectation(self):
        for case in oracle.gen():
            self.assertIn("expect", case, case["id"])
            self.assertTrue(
                case["expect"] is None or set(case["expect"]) in ({"pick", "count"}, {"pick", "count", "versions"}),
                case["id"],
            )

    def test_the_generated_set_walks_versions_both_ways(self):
        fams = [c for c in oracle.gen() if c["family"].startswith("versions")]
        self.assertTrue(any(c["expect"] and "versions" in c["expect"] for c in fams))
        self.assertTrue(any(c["expect"] and "versions" not in c["expect"] for c in fams))


class TheModelReadsTheDocs(unittest.TestCase):
    """Rows transcribed from the decision tables in docs/title-resolution.md."""

    def test_admission_by_example(self):
        for titles, cand, want in [
            (["[Oshi no Ko]"], "My Star: Season 2", False),
            (["86 Part 2"], "Eighty Six: 2nd Season", True),
            (["Show"], "Show 2", False),
            (["Show"], "Show 2 Part 1", False),
            (["ショー"], "ショー２", False),
            (["Show"], "Show 1", True),
            (["Show 2"], "Show 2", True),
            (["Kaiju No. 8"], "Kaiju No. 8", True),
            (["怪獣８号"], "Kaiju No. 8", True),
            (["Mob Psycho 100"], "Mob Psycho 100", True),
            (["Show 2"], "Show20", False),
            (["Mob Psycho 100 II"], "Mob Psycho 1000 Part 2", False),
            (["86 Part 2"], "861", False),
            (["Kaiju No. 8 Season 2"], "Kaiju No. 8 2nd Season", True),
            (["Show 2"], "Show 2 (2020)", True),
            (["Show 2", "Other Name 20"], "Show20", False),
            (["Show 2", "Show 20"], "Show20", True),
            (["Show"], "Show Season 2 (2024)", False),
            (["Show"], "Show Part 2 (TV)", False),
            (["Show Season 2"], "Show Season 2 (TV)", True),
            (["Show"], "Show Seventh Season", False),
            (["Show Seventh Season"], "Show 7th Season", True),
            (["Show Season Seven"], "Show 7th Season", True),
            (["Show VII"], "Show Season 7", True),
            (["Spy x Family"], "Spy x Family", True),
            (["ショー 第十期"], "Show Season 10", True),
            (["ショー 第十二部"], "Show Part 12", True),
            (["怪獣八号"], "怪獣八十号", False),
            (["ショー 第〇期"], "Show Season 0", True),
            (["Show"], "Show Season 0", False),
        ]:
            self.assertEqual(oracle.admitted(titles, cand), want, (titles, cand))

    def test_the_entry_before_and_the_season_before(self):
        for titles, cand, pickable in [
            (["Show 2"], "Show", False),
            (["Show 2"], "Show Season 1", False),
            (["Show 2"], "Show First Half", False),
            (["Show 2"], "Show: Season 1", False),
            (["Show: Season 2"], "Show Season 1", False),
            (["Steins;Gate 0"], "Steins Gate", False),
            (["Show Part 2"], "Show Season 1", False),
            (["Show Seventh Season"], "Show", False),
            (["Show 2"], "Show Season 1 (2020)", False),
            (["Show 2"], "Show (TV)", False),
            (["Show 2"], "Show (Shin Series)", True),
            (["Gintama 2"], "Gintama'", True),
            (["Steins;Gate 0"], "Steins;Gate Zero", True),
            (["Show 2", "Show (2020)"], "Show", True),
            (["86"], "Eighty Six", True),
            (["ショー 第十期"], "Show Season 1", True),
            (["Show Season 3 Part 2"], "Show Season 2 Part 2", False),
            (["Show Season 3 Part 2"], "Show Season 2", False),
            (["Show 3 Part 2"], "Show 2 Part 2", False),
            (["Show Season 3", "Show Part 2"], "Show Season 2", False),
            (["86 Part 2"], "Eighty Six: 2nd Season", True),
            (["Kaiju No. 8 Season 2"], "Kaiju No. 8 2nd Season", True),
        ]:
            self.assertEqual(oracle.pickable(titles, cand), pickable, (titles, cand))

    def test_the_pools_the_table_answers_without_a_broad_listing(self):
        for titles, count, rows, want in [
            (["ショー 第十期"], E, [("x", "Show Season 10", E)], pick("x")),
            (["ショー 第十二部"], E, [("x", "Show Part 12", E)], pick("x")),
            (["怪獣八号"], E, [("x", "Kaiju No. 8", E)], pick("x")),
            (["怪獣八号"], E, [("x", "怪獣八十号", E)], None),
            (["ショー 第〇期"], E, [("x", "Show Season 0", E)], pick("x")),
            (["Show Seventh Season"], E, [("x", "Show 7th Season", E)], pick("x")),
            (["Show"], E, [("x", "Show Seventh Season", E)], None),
            (["Show Seventh Season"], E, [("x", "Show", E)], None),
            (["Show 2"], E, [("x", "Show", E)], None),
            (["Show Part 2"], E, [("x", "Show Season 1", E)], None),
            (["Show"], E, [("x", "Show Season 2 (2024)", E)], None),
            (["Show Season 2"], E, [("x", "Show Season 2 (TV)", E)], pick("x")),
            (["Hunter x Hunter"], None, [("new", "Hunter x Hunter (2011)", 148), ("old", "Hunter x Hunter", 62)], pick("old", 62)),
            (["Fruits Basket"], 25, [("new", "Fruits Basket (2019)", 25), ("old", "Fruits Basket", 25)], pick("old", 25)),
            (["Show (2011) Season 2"], E, [("a", "Show Season 2", E), ("b", "Show Season 2 (2011)", E)], pick("b")),
            (["Show 2"], E, [("x", "Show Season 1 (2020)", E)], None),
            (["Show 2"], E, [("x", "Show (Shin Series)", E)], pick("x")),
            (["Gintama 2"], E, [("x", "Gintama'", E)], pick("x")),
            (["Show 2", "Show (2020)"], E, [("x", "Show", E)], pick("x")),
            (["Show 2"], E, [("a", "Show 2", E), ("b", "Show", E)], pick("a")),
            (["86"], E, [("x", "Eighty Six", E)], pick("x")),
            (["Kaiju No. 8 Season 2"], E, [("x", "Kaiju No. 8 2nd Season", E)], pick("x")),
            (["Show 2"], E, [("x", "Show 2 (2020)", E)], pick("x")),
            (["Show 2", "Other Name 20"], E, [("x", "Show20", E)], None),
            (["Show"], E, [("x", "Show 2", E)], None),
            (["Show: Arc"], E, [("a", "Show 2", E), ("b", "Show", E)], pick("b")),
            # the probe head: five other entries are never charged to the five
            (
                ["Show 2"],
                E,
                [("a", "Show", E), ("b", "Show Season 1", E), ("c", "Show Part 1", E), ("d", "Show First Season", E), ("e", "Show First Half", E), ("own", "Show 2", E)],
                pick("own"),
            ),
            # five pickable hits fill the five: the sixth is never considered
            (
                ["Show 2"],
                E,
                [(f"r{i}", f"Show 2 Arc {c}", 50) for i, c in enumerate("ABCDE")] + [("own", "Show 2", E)],
                None,
            ),
            # two same-length cours: the one whose part agrees
            (["Show 2"], E, [("a", "Show 2nd Season Part 2", E), ("b", "Show Season 2", E)], pick("b")),
        ]:
            self.assertEqual(oracle.predict(pool(titles, count, rows)), want, (titles, count, rows))

    def test_copies_under_a_version_tag_are_versions_of_the_pick(self):
        def versions(slug, *vs, count=E):
            return {"pick": slug, "count": count, "versions": [list(v) for v in vs]}

        both = [("p", "Show", E), ("u", "Show (Uncensored)", E)]
        for titles, count, rows, want in [
            # the docs' table, row by row where a generated pool can say it
            (["Show"], E, both, versions("u", ("p", "standard"), ("u", "uncensored"))),
            (["Show"], E, list(reversed(both)), versions("u", ("u", "uncensored"), ("p", "standard"))),
            (["Other Name", "Show"], E, both, versions("u", ("p", "standard"), ("u", "uncensored"))),
            (["Show"], E, [("p", "Show", E), ("u", "Show [UNCENSORED]", E)], versions("u", ("p", "standard"), ("u", "uncensored"))),
            (["Show"], E, [("p", "Show", E), ("u", "Show （Uncensored）", E)], versions("u", ("p", "standard"), ("u", "uncensored"))),
            (["Show 2nd Season"], E, [("p", "Show 2nd Season", E), ("u", "Show 2nd Season (Uncensored)", E)], versions("u", ("p", "standard"), ("u", "uncensored"))),
            (["Show"], E, [("p", "Show", E), ("u", "Show (Uncensored)", 11)], pick("p")),
            (["Show"], E, [("p", "Show", 11), ("u", "Show (Uncensored)", 13)], pick("p", 11)),
            (["Show"], E, [("p", "Show", E), ("u", "Show Uncensored", E)], pick("p")),
            (["Show"], E, [("p", "Show", E), ("u", "Show Other (Uncensored)", E)], pick("p")),
            (["Show"], E, [("p", "Show", E), ("u", "Show (Uncensored)", E), ("q", "Show", E)], pick("p")),
            (["Show"], E, [("u", "Show (Uncensored)", E)], pick("u")),
            (["Show"], E, [("p", "Show Season 2", E), ("u", "Show Season 2 (Uncensored)", E)], pick("u")),
            (["Show"], None, both, pick("p")),
        ]:
            self.assertEqual(oracle.predict(pool(titles, count, rows)), want, (titles, count, rows))
        # a count and no year: the tagged copy is named by nothing
        self.assertEqual(oracle.predict(pool(["Show"], E, list(reversed(both)), year=None)), pick("p"))

    def test_the_countless_pick_takes_only_what_something_vouches_for(self):
        # with the entry's year, every listing's page year (the driver writes
        # 2020 on each) vouches for it: part agreement, then provider order
        self.assertEqual(oracle.predict(pool(["Show"], None, [("z", "Zeta Thing", 24), ("x", "Show Recap", E)])), pick("z", 24))
        # the exact title is taken before anything else
        self.assertEqual(oracle.predict(pool(["Show"], None, [("z", "Zeta Thing", 24), ("x", "Show", E)])), pick("x"))
        # with no year, only a hit the entry names: the title reads as one of
        # the entry's titles, under its name, with the same words past it
        for rows, want in [
            ([("z", "Zeta Thing", 24), ("x", "Show Recap", E)], None),
            ([("z", "Zeta Thing", 24), ("x", "Show (2020)", E)], pick("x")),
            ([("z", "Zeta Thing", 24), ("x", "Show Part 1", E)], pick("x")),
            ([("x", "Show Season 1", E)], pick("x")),
            ([("x", "Show", E)], pick("x")),
        ]:
            self.assertEqual(oracle.predict(pool(["Show"], None, rows, year=None)), want, rows)
        for titles, rows, want in [
            (["Show 2"], [("x", "Show 2nd Season", E)], pick("x")),
            (["Show 2"], [("x", "Show 2nd Season: Extra Arc", E)], None),
            (["Zzyzx Alias", "Show 2"], [("x", "Show Season 2", E)], pick("x")),
            (["Show 2nd Season"], [("x", "Other Show Season 2", E)], None),
            (["Gintama'"], [("x", "Gintama", E)], None),
            (["Hunter x Hunter"], [("x", "Hunter x Hunter (2011)", E)], pick("x")),
            (["Film (2019)"], [("x", "Film (2020)", E)], None),
            (["Dragon Ball Super: Ginga Patrol", "Dragon Ball Super: The Galactic Patrol"], [("x", "Dragon Ball Super: The Galactic Patrol", E)], pick("x")),
            (["ショウ ゼータ"], [("x", "Show Zeta", E)], None),
        ]:
            self.assertEqual(oracle.predict(pool(titles, None, rows, year=None)), want, (titles, rows))

    def test_a_count_with_no_year_takes_only_what_the_title_or_a_name_vouches_for(self):
        # "A count with no year is no identity either": with a count above
        # one and no year, a hit is picked only when it is the title
        # searched or the entry names it; any other is refused, and the
        # pool is the refusal by title, not the clean miss.
        other = ("o", "Other", E)
        for titles, rows, want in [
            (["Show: Arc"], [other], None),
            (["Show: Arc"], [other, ("s", "Show: Arc", E)], pick("s")),
            (["Show: Arc", "Shou Arc"], [other, ("a", "Shou Arc", E)], pick("a")),
            (
                ["Yi Ren Zhi Xia 6th Season", "The Outcast 6th Season"],
                [("s5", "The Outcast 5th Season", 12), ("s6", "The Outcast 6", 26), ("m", "The Outcast Movie: The Rusted Blade Resurfaces", 1), ("f", "Yi Ren Zhi Xia: Fanwai Pian", 2)],
                pick("s6", 26),
            ),
        ]:
            count = 26 if titles[0].startswith("Yi Ren") else E
            self.assertEqual(oracle.predict(pool(titles, count, rows, year=None)), want, (titles, rows))
        self.assertEqual(oracle.refusal_expected(pool(["Show: Arc"], E, [other], year=None)), ("NoResults", False))
        # a dated entry whose card carries the entry's name takes the hit
        # on its year and count
        self.assertEqual(oracle.predict(pool(["Show: Arc"], E, [other])), pick("o"))
        # and a countless one with no year is the clean miss, as before
        self.assertIsNone(oracle.predict(pool(["Show: Arc"], None, [other], year=None)))
        self.assertEqual(oracle.refusal_expected(pool(["Show: Arc"], None, [other], year=None)), ("NoResults", True))

    def test_a_year_and_a_count_take_only_what_a_name_vouches_for(self):
        # "A year and a count are no identity either": with a count above
        # one and a year, a hit is picked only when it is the title
        # searched, or a name of the show — its listing's title, its
        # card's romanized name, a name its page gives — is named by the
        # entry or shares one of its titles' names; any other is refused,
        # and the pool is the refusal by title, not the clean miss.
        other = ("o", "Other", E)
        bare = {"jnames": {}}
        for titles, rows, names, want in [
            (["Show: Arc"], [other], bare, None),
            (["Show: Arc"], [other, ("s", "Show: Arc", E)], bare, pick("s")),
            (["Show: Arc", "Shou Arc"], [other], {"jnames": {"o": "Shou Arc"}}, pick("o")),
            (["Show: Arc"], [other], {"jnames": {}, "page_names": {"o": ["Show: Arc"]}}, pick("o")),
            (["Show: Arc 2"], [("g", "Show: Arc - Second Half War", E)], bare, pick("g")),
            (["Show: Arc Part 2"], [("j", "Show Part 6: Arc Part 2", E)], bare, pick("j")),
            (["Show: Arc"], [("a", "Arcadia Show", E)], bare, None),
            (
                ["HenSemi (TV)", "変ゼミ", "Hentai Seiri Seminar", "Hen Zemi", "Abnormal Physiology Seminar"],
                [("m", "Medaka Box Abnormal", 12)],
                {"jnames": {"m": "Medaka Box Abnormal"}, "page_names": {"m": ["めだかボックス アブノーマル"]}},
                None,
            ),
        ]:
            self.assertEqual(oracle.predict(pool(titles, E, rows, **names)), want, (titles, rows, names))
        self.assertEqual(oracle.refusal_expected(pool(["Show: Arc"], E, [other], **bare)), ("NoResults", False))
        # a name is started at a word's end, or with a digit or 第 glued
        # to it — no digit when the name itself ends in one
        self.assertEqual(oracle.predict(pool(["Show"], E, [other], jnames={}, page_names={"o": ["Show2"]})), pick("o"))
        self.assertEqual(oracle.predict(pool(["ショー"], E, [other], jnames={}, page_names={"o": ["ショー第2期"]})), pick("o"))
        self.assertIsNone(oracle.predict(pool(["Show"], E, [other], jnames={}, page_names={"o": ["Showtime"]})))
        self.assertIsNone(oracle.predict(pool(["86"], E, [other], jnames={}, page_names={"o": ["861"]})))
        # a refused hit the airing rescue would have taken — from the
        # entry's year, shorter than the entry — is no clean miss either
        self.assertEqual(oracle.refusal_expected(pool(["Show"], E, [("x", "Other", 2)], **bare)), ("NoResults", False))
        self.assertEqual(oracle.refusal_expected(pool(["Show"], E, [("x", "Other", 40)], **bare)), ("NoResults", True))
        # a name of no words names no show
        self.assertFalse(oracle.shares_name(["Season 2"], "Other Show"))
        # the countless pick and an entry with no year read as before
        self.assertEqual(oracle.predict(pool(["Show: Arc"], None, [other], **bare)), pick("o"))
        self.assertIsNone(oracle.predict(pool(["Show: Arc"], E, [other], year=None, jnames={"o": "Show: Arc"})))

    def test_the_probe_head_stops_at_the_fifth_that_could_be_picked(self):
        rows = [(f"r{i}", f"Show 2 Arc {c}", 50) for i, c in enumerate("ABCDE")] + [("x", "Show 3", E)]
        picked, evidence = oracle.head(["Show 2"], rows, True)
        self.assertEqual([r[0] for r in picked], ["r0", "r1", "r2", "r3", "r4"])
        self.assertEqual(evidence, [], "a hit behind the fifth that could be picked is never read")
        self.assertTrue(oracle.clean_miss_expected(pool(["Show 2"], E, rows)))
        before = [("x", "Show 3", E)] + rows[:5]
        self.assertEqual([r[0] for r in oracle.head(["Show 2"], before, True)[1]], ["x"])
        self.assertFalse(oracle.clean_miss_expected(pool(["Show 2"], E, before)))

    def test_a_japanese_division_reads_alike_glued_or_apart(self):
        self.assertTrue(oracle.names(["ショー 第2期"], "ショー第2期"))
        self.assertTrue(oracle.names(["ショー第2期"], "ショー 第2期"))
        self.assertEqual(oracle.Reading("ショー第2期第2部").ords, [2, 2])
        self.assertEqual(oracle.Reading("ショー第2期第2部").name, ["ショー"])
        self.assertEqual(oracle.Reading("ショー 第2期 第2部").ords, [2, 2])
        self.assertEqual(oracle.Reading("ショー２").ords, [2])
        self.assertEqual(oracle.Reading("ショー２").name, ["ショー"])
        self.assertEqual(oracle.Reading("怪獣８号").ords, [], "the ８ inside 怪獣８号 is no division")
        self.assertEqual(oracle.Reading("ショー 第十期").ords, [10])

    def test_a_span_through_the_first_part_reads_as_the_whole(self):
        self.assertEqual(oracle.Reading("Haikyuu!!: To the Top (Part 1+2)").ords, [])
        self.assertTrue(oracle.names(["Haikyuu!!: To the Top"], "Haikyuu!!: To the Top (Part 1+2)"))
        self.assertEqual(oracle.Reading("Show Part 2+3").ords, [(2, 3)])

    def test_a_marker_season_below_an_entry_s_bare_number_is_the_season_before(self):
        self.assertFalse(oracle.pickable(["Show 3 Part 2"], "Show Season 2 Part 2"))
        self.assertFalse(oracle.pickable(["Show 3"], "Show Season 2"))
        self.assertTrue(oracle.pickable(["Show 2"], "Show Season 2"))
        self.assertTrue(oracle.pickable(["Show 3 Part 2"], "Show 3rd Season Part 2"))

    def test_parts_that_could_stitch_are_tabled_not_predicted(self):
        for rows in [
            [("a", "Show", 4), ("b", "Show Part 2", 3)],
            [("a", "Show", 4), ("b", "Show Part 2", 4), ("c", "Show Part 3", 4)],
            [("a", "Show", 11), ("b", "Show 2nd Stage", 1)],
            [("a", "Show", 1), ("b", "Show 2nd Stage", 11)],
        ]:
            self.assertIs(oracle.predict(pool(["Show"], E, rows)), oracle.NOT_MODELLED, rows)
        # two whole listings of the entry's length together explain nothing
        self.assertEqual(oracle.predict(pool(["Show"], E, [("a", "Show", E), ("b", "Show Part 1", E)])), pick("a"))
        # a three-digit stem keeps its number: "Mob Psycho 1 …" is no kin of it
        self.assertIsNone(oracle.predict(pool(["Mob Psycho 100"], E, [("w", "Mob Psycho 100", 24), ("x", "Mob Psycho 1 Part 2", E)])))

    def test_a_longer_number_glued_to_a_japanese_word_is_a_word_of_its_own(self):
        # the docs read a one- or two-digit number glued to a Japanese word
        # as a division; a longer one is no division, and glued or apart it
        # is the same word
        self.assertEqual(oracle.Reading("ショー200").name, ["ショー", "200"])
        self.assertEqual(oracle.Reading("ショー 200").name, ["ショー", "200"])
        self.assertTrue(oracle.names(["ショー 200"], "ショー200"))
        self.assertEqual(oracle.Reading("怪獣８号").name, ["怪獣8号"])

    def test_a_refused_lead_or_a_first_part_stitches_nothing(self):
        # a candidate the titles refuse may stitch as a later part, never as
        # the lead, and a first part completes no bare title: both pools are
        # decided by the rules the predicates read
        self.assertIsNone(oracle.predict(pool(["Show"], E, [("a", "Show 2", 6), ("b", "Show 2 Part 2", 6)])))
        self.assertIsNone(oracle.predict(pool(["Show"], E, [("a", "Show", 6), ("b", "Show Part 1", 6)])))

    def test_a_half_names_its_ordinal_and_is_no_sequel_guard_marker(self):
        self.assertTrue(oracle.admitted(["Show 2nd Half"], "Show Season 2"))
        self.assertTrue(oracle.admitted(["Show Second Half"], "Show Season 2"))
        self.assertFalse(oracle.later_part_of(["Show 2nd Half"], "Show"))
        self.assertFalse(oracle.pickable(["Show 2nd Half"], "Show"), "the entry before, by every title naming a later division")

    def test_the_pools_a_broad_listing_shapes_are_tabled_not_predicted(self):
        self.assertIs(oracle.predict(pool(["Show 2"], E, [("w", "Show 2", 24), ("x", "Show 2 Part 2", E)])), oracle.NOT_MODELLED)
        self.assertIs(oracle.predict(pool(["Show 2"], E, [("d", "Show 2", E)], dead=["d"])), oracle.NOT_MODELLED)

    def test_the_tabled_families_agree_with_the_predicates(self):
        """Every generated pool the predicates can read predicts the
        expectation its family tabled from the docs: the two readings of
        the same document agree."""
        covered = 0
        for case in oracle.gen():
            want = oracle.predict(case)
            if want is oracle.NOT_MODELLED:
                continue
            covered += 1
            self.assertEqual(want, case["expect"], case)
        self.assertGreater(covered, 10000)

    def test_the_hand_written_cases_agree_with_the_predicates(self):
        covered = 0
        for name in sorted(os.listdir(os.path.join(HERE, "cases"))):
            with open(os.path.join(HERE, "cases", name), encoding="utf-8") as fh:
                for case in json.load(fh):
                    want = oracle.predict(case)
                    if want is oracle.NOT_MODELLED:
                        continue
                    covered += 1
                    self.assertEqual(want, case["expect"], case)
        self.assertGreater(covered, 60)


def outcome_of(case):
    """The outcome the driver would report for a pool the picker agrees on:
    the pick, or a refusal carrying the clean-miss verdict the docs give."""
    expect = case["expect"]
    if expect is None:
        err, clean = oracle.refusal_expected(case)
        return {"err": err, "clean_miss": clean}
    got = {"ok": expect["pick"], "count": expect["count"], "episodes": [[k, k, None] for k in range(1, expect["count"] + 1)]}
    if "versions" in expect:
        got["versions"] = expect["versions"]
    return got


REFUSED = {"refused": "NoResults", "clean_miss": True}


class Comparison(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cases = [
            dict(pool(["Show"], E, [("x", "Show", E)]), id="a", expect=pick("x")),
            dict(pool(["Show"], E, [("x", "Show 2", E)]), id="b", expect=None),
            dict(pool(["Show 2"], E, [("x", "Show", E)]), id="c", expect=None),
        ]
        self.got = {c["id"]: outcome_of(c) for c in self.cases}

    def tearDown(self):
        self.tmp.cleanup()

    def run_cmp(self, allowlist=None, got=None):
        got = self.got if got is None else got
        paths = {}
        for name, data in [("cases", self.cases), ("out", [{"id": i, "got": g} for i, g in got.items()]), ("allow", allowlist)]:
            if data is None:
                continue
            paths[name] = os.path.join(self.tmp.name, name + ".json")
            with open(paths[name], "w", encoding="utf-8") as fh:
                json.dump(data, fh)
        buf = io.StringIO()
        with redirect_stdout(buf):
            code = oracle.compare(paths["cases"], paths["out"], paths.get("allow"))
        return code, buf.getvalue()

    def test_agreement_everywhere_passes(self):
        code, out = self.run_cmp()
        self.assertEqual(code, 0, out)
        self.assertIn("disagreements 0", out)

    def test_a_refused_fitting_hit_must_leave_no_clean_miss(self):
        got = dict(self.got, b={"err": "NoResults", "clean_miss": True})
        code, out = self.run_cmp(got=got)
        self.assertEqual(code, 1)
        self.assertIn("b", out)
        got = dict(self.got, c={"err": "NoResults", "clean_miss": False})
        code, out = self.run_cmp(got=got)
        self.assertEqual(code, 1)

    def test_an_unlisted_disagreement_fails_and_is_named(self):
        got = dict(self.got, a={"err": "NoResults", "clean_miss": True})
        code, out = self.run_cmp(got=got)
        self.assertEqual(code, 1)
        self.assertIn("a [test]", out)
        self.assertIn("not in the allowlist", out)

    def test_a_listed_disagreement_with_the_recorded_outcome_passes(self):
        got = dict(self.got, a={"err": "NoResults", "clean_miss": True})
        code, out = self.run_cmp(allowlist=[{"id": "a", "got": REFUSED, "reason": "known"}], got=got)
        self.assertEqual(code, 0, out)
        self.assertIn("accepted 1", out)

    def test_a_listed_disagreement_that_moved_fails(self):
        got = dict(self.got, a=outcome_of(dict(self.cases[0], expect=pick("x", 11))))
        code, out = self.run_cmp(allowlist=[{"id": "a", "got": REFUSED, "reason": "known"}], got=got)
        self.assertEqual(code, 1)
        self.assertIn("recorded", out)

    def test_a_stale_allowlist_entry_fails(self):
        code, out = self.run_cmp(allowlist=[{"id": "a", "got": REFUSED, "reason": "fixed since"}])
        self.assertEqual(code, 1)
        self.assertIn("stale", out)

    def test_an_allowlist_entry_for_no_case_fails(self):
        code, out = self.run_cmp(allowlist=[{"id": "zz", "got": REFUSED, "reason": "gone"}])
        self.assertEqual(code, 1)
        self.assertIn("zz", out)

    def test_an_entry_without_a_reason_fails(self):
        got = dict(self.got, a={"err": "NoResults", "clean_miss": True})
        code, out = self.run_cmp(allowlist=[{"id": "a", "got": REFUSED}], got=got)
        self.assertEqual(code, 1)
        self.assertIn("reason", out)

    def test_a_pick_must_list_the_episodes_the_docs_give_it(self):
        # every generated listing numbers its rows 1..n; a pick plays the
        # first `count` of them — a cut its entry's own episodes from the
        # start, an uncut pick the whole listing
        for episodes in ([[k, k, None] for k in range(13, 25)], list(reversed(outcome_of(self.cases[0])["episodes"])), [[k + 100, k, None] for k in range(1, 13)]):
            got = dict(self.got, a={"ok": "x", "count": E, "episodes": episodes})
            code, out = self.run_cmp(got=got)
            self.assertEqual(code, 1, episodes)
            self.assertIn("a [test]", out)

    def test_an_allowlist_entry_records_a_pick_s_episodes(self):
        got = dict(self.got, a={"ok": "x", "count": E, "episodes": [[k, k, None] for k in range(13, 25)]})
        listed = {"pick": "x", "count": E, "episodes": [[k, k, None] for k in range(13, 25)]}
        code, out = self.run_cmp(allowlist=[{"id": "a", "got": listed, "reason": "known"}], got=got)
        self.assertEqual(code, 0, out)

    def test_a_refusal_must_name_the_error_the_docs_give_it(self):
        # nothing matching is NoResults; a transport failure is the network
        # failure it is
        code, out = self.run_cmp(got=dict(self.got, c={"err": "Network", "clean_miss": True}))
        self.assertEqual(code, 1, out)

    def test_a_dead_listing_that_blocks_the_winner_leaves_a_transient_verdict(self):
        dead = dict(pool(["Show 2"], E, [("dead", "Show 2nd Season", E), ("own", "Show Season 2", E)], dead=["dead"]), id="d", expect=None)
        self.cases.append(dead)
        self.assertEqual(outcome_of(dead), {"err": "Network", "clean_miss": False})
        for wrong in ({"err": "NoResults", "clean_miss": True}, {"err": "NoResults", "clean_miss": False}, {"err": "Network", "clean_miss": True}):
            code, out = self.run_cmp(got=dict(self.got, d=wrong))
            self.assertEqual(code, 1, wrong)
        code, out = self.run_cmp(got=dict(self.got, d={"err": "Network", "clean_miss": False}))
        self.assertEqual(code, 0, out)

    def test_a_pick_must_carry_the_versions_the_docs_give_it(self):
        two = dict(
            pool(["Show"], E, [("p", "Show", E), ("u", "Show (Uncensored)", E)]),
            id="v",
            expect={"pick": "u", "count": E, "versions": [["p", "standard"], ["u", "uncensored"]]},
        )
        self.cases.append(two)
        rows = [[k, k, None] for k in range(1, E + 1)]
        right = {"ok": "u", "count": E, "episodes": rows, "versions": [["p", "standard"], ["u", "uncensored"]]}
        code, out = self.run_cmp(got=dict(self.got, v=right))
        self.assertEqual(code, 0, out)
        for wrong in (
            {"ok": "u", "count": E, "episodes": rows},
            {"ok": "u", "count": E, "episodes": rows, "versions": [["u", "uncensored"], ["p", "standard"]]},
            {"ok": "p", "count": E, "episodes": rows, "versions": [["p", "standard"], ["u", "uncensored"]]},
        ):
            code, out = self.run_cmp(got=dict(self.got, v=wrong))
            self.assertEqual(code, 1, wrong)
        # and a pick the docs give no versions must carry none
        code, out = self.run_cmp(got=dict(self.got, a=dict(self.got["a"], versions=[["x", "standard"], ["y", "uncensored"]])))
        self.assertEqual(code, 1, out)

    def test_a_panic_or_a_missing_outcome_fails(self):
        code, out = self.run_cmp(got=dict(self.got, a={"panic": True}))
        self.assertEqual(code, 1)
        self.assertIn("panic", out)
        code, out = self.run_cmp(got={k: v for k, v in self.got.items() if k != "a"})
        self.assertEqual(code, 1)
        self.assertIn("no outcome", out)

    def test_the_command_line_round_trip(self):
        cases = os.path.join(self.tmp.name, "cases.json")
        out = os.path.join(self.tmp.name, "out.json")
        gen = subprocess.run([sys.executable, "-I", os.path.join(HERE, "oracle.py"), "gen", cases], capture_output=True, text=True)
        self.assertEqual(gen.returncode, 0, gen.stderr)
        with open(cases, encoding="utf-8") as fh:
            generated = json.load(fh)
        self.assertEqual(generated, oracle.gen())
        # every pool agrees but one picked pool, which the allowlist carries
        at, first = next((i, c) for i, c in enumerate(generated) if c["expect"])
        outcomes = [{"id": c["id"], "got": outcome_of(c)} for c in generated]
        outcomes[at]["got"] = {"err": "Network", "clean_miss": False}
        with open(out, "w", encoding="utf-8") as fh:
            json.dump(outcomes, fh)
        allow = os.path.join(self.tmp.name, "allow.json")
        with open(allow, "w", encoding="utf-8") as fh:
            json.dump([{"id": first["id"], "got": {"refused": "Network", "clean_miss": False}, "reason": "a probe dies here"}], fh)
        cmp = subprocess.run(
            [sys.executable, "-I", os.path.join(HERE, "oracle.py"), "cmp", cases, out, "--allowlist", allow],
            capture_output=True,
            text=True,
        )
        self.assertEqual(cmp.returncode, 0, cmp.stdout)
        self.assertIn("disagreements 1 accepted 1", cmp.stdout)
        # without the allowlist the same outcomes fail, naming the pool
        cmp = subprocess.run([sys.executable, "-I", os.path.join(HERE, "oracle.py"), "cmp", cases, out], capture_output=True, text=True)
        self.assertEqual(cmp.returncode, 1, cmp.stdout)
        self.assertIn(first["id"], cmp.stdout)


if __name__ == "__main__":
    unittest.main()
