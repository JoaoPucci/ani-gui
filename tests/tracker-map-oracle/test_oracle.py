"""The model's own tests: the rows of the case table in
docs/title-resolution.md predict as the table says; the generated set
is reproducible and covers every family; the comparison refuses what it
must."""

import copy
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import oracle  # noqa: E402
from oracle import evidence, kitsu, predict, show, targets  # noqa: E402


def ranma():
    return evidence(
        kitsu(186, 161, "1989-04-15", "1992-09-25"),
        show(210, 18, True, "1989-04-15", "1989-09-16"),
        chain=[show(149939, 143, True, "1989-10-20", "1992-09-25")],
    )


class CaseTable(unittest.TestCase):
    """One test per row of the docs' table."""

    def test_agreeing_counts_are_identity(self):
        ev = evidence(kitsu(1, 12), show(2, 12))
        self.assertEqual(predict(ev), oracle.identity("counts_agree"))

    def test_an_unknown_count_is_identity(self):
        self.assertEqual(predict(evidence(kitsu(1, None), show(2, 12)))["why"], "count_unknown")
        self.assertEqual(predict(evidence(kitsu(1, 12), show(2, None)))["why"], "count_unknown")

    def test_a_chain_files_each_segment_under_its_entry(self):
        m = predict(ranma())
        self.assertEqual(m["shape"], "chain")
        t = targets(m, 161)
        self.assertEqual(t[17], [210, 18, True, []])
        self.assertEqual(t[18], [149939, 1, False, [[210, 18, True]]])
        self.assertEqual(t[160], [149939, 143, True, [[210, 18, True]]])
        self.assertIsNone(t[161])

    def test_a_chain_outside_kitsu_s_run_is_identity(self):
        ev = ranma()
        ev["chain"][0]["end"] = "1993-01-01"
        self.assertEqual(predict(ev)["why"], "unexplained")

    def test_a_segment_past_kitsu_s_run_is_inside_where_kitsu_s_episodes_date_it(self):
        ev = evidence(
            kitsu(1633, 78, "2003-04-06", "2004-03-28"),
            show(1816, 52, True, "2003-04-06", "2004-03-28"),
            chain=[show(129610, 26, True, "2005-09-10", "2006-05-06")],
        )
        self.assertEqual(predict(ev)["why"], "unexplained")
        ev["kitsu_airdates"] = {"53": "2005-09-10", "78": "2006-05-06"}
        self.assertEqual(predict(ev)["shape"], "chain")
        ev["kitsu_airdates"] = {"53": "2005-09-10"}
        self.assertEqual(predict(ev)["why"], "unexplained")
        # Wholly past Kitsu's end, the start needs its date too.
        ev["kitsu_airdates"] = {"78": "2006-05-06"}
        self.assertEqual(predict(ev)["why"], "unexplained")

    def test_a_run_two_days_past_kitsu_s_is_inside_and_three_is_not(self):
        def ev(end):
            return evidence(
                kitsu(1, 36, "2018-08-10", "2020-09-30"),
                show(2, 24, True, "2018-08-12", "2019-10-15"),
                chain=[show(3, 12, True, "2020-04-15", end)],
            )
        self.assertEqual(predict(ev("2020-10-02"))["shape"], "chain")
        self.assertEqual(predict(ev("2020-10-03"))["why"], "unexplained")

    def test_a_claimed_segment_is_identity(self):
        ev = ranma()
        ev["claimed"] = [149939]
        self.assertEqual(predict(ev)["why"], "unexplained")

    def test_a_shorter_entry_with_no_chain_is_identity(self):
        ev = ranma()
        ev["chain"] = []
        self.assertEqual(predict(ev)["why"], "unexplained")

    def test_shared_entry_parts_take_it_in_date_order(self):
        parts = [kitsu(47132, 1, "2023-11-04", "2023-11-04"), kitsu(46038, 1, "2023-03-03", "2023-03-03")]
        first = predict(evidence(parts[1], show(51535, 2), sharing=parts))
        second = predict(evidence(parts[0], show(51535, 2), sharing=parts))
        self.assertEqual(targets(first, 1)[0], [51535, 1, False, []])
        self.assertEqual(targets(second, 1)[0], [51535, 2, True, []])

    def test_split_parts_take_the_entry_after_the_parts_before(self):
        parts = [kitsu(46312, 13, "2024-07-07", ""), kitsu(49244, 13, "2024-10-06", "")]
        m = predict(evidence(parts[1], show(153406, 26), parts=parts))
        self.assertEqual(m["shape"], "split_parts")
        self.assertEqual(targets(m, 13)[0], [153406, 14, False, []])
        self.assertEqual(targets(m, 13)[12], [153406, 26, True, []])

    def test_a_prologue_shifts_every_episode_and_the_finale_completes(self):
        listed = ["0"] + [str(i) for i in range(1, 26)]
        m = predict(evidence(kitsu(7882, 12), show(19603, 13), listed=listed))
        self.assertEqual(m["shape"], "extras")
        t = targets(m, 12)
        self.assertEqual(t[0], [19603, 2, False, []])
        self.assertEqual(t[11], [19603, 13, True, []])

    def test_several_recaps_shift_the_episodes_after_each(self):
        listed = [str(i) for i in range(1, 25)] + ["0", "6.5", "12.5", "18.5", "18.5"]
        m = predict(evidence(kitsu(1, 24), show(2, 28), listed=listed))
        self.assertEqual(m["segments"][0]["extras_after"], [0, 6, 12, 18])
        t = targets(m, 24)
        self.assertEqual([t[k - 1][1] for k in (1, 6, 7, 13, 19, 24)], [2, 7, 9, 16, 23, 28])

    def test_extras_that_do_not_account_for_the_surplus_are_identity(self):
        two_for_one = evidence(kitsu(1, 12), show(2, 13), listed=["0", "6.5", "1"])
        self.assertEqual(predict(two_for_one)["why"], "unexplained")
        unlisted = evidence(kitsu(1, 12), show(2, 13))
        self.assertEqual(predict(unlisted)["why"], "unexplained")

    def test_haruhi_s_unexplained_surplus_is_identity(self):
        ev = evidence(kitsu(3579, 14, "2009-05-22", "2009-09-11"), show(4382, 28), listed=[str(i) for i in range(1, 29)])
        self.assertEqual(predict(ev)["why"], "unexplained")

    def test_two_rules_disagreeing_conflict_and_agreeing_are_one(self):
        parts = [kitsu(49900, 12, "2025-07-09", ""), kitsu(50027, 1, "2025-10-02", "")]
        conflict = evidence(parts[0], show(173335, 13), parts=parts, listed=["0", "1"])
        self.assertEqual(predict(conflict)["why"], "conflict")
        agree = evidence(parts[0], show(173335, 13), parts=parts, sharing=parts)
        self.assertEqual(predict(agree)["shape"], "shared_entry")

    def test_a_later_part_is_not_filed_where_its_first_part_conflicts(self):
        parts = [kitsu(49900, 12, "2025-07-09", ""), kitsu(50027, 1, "2025-10-02", "")]
        later = evidence(parts[1], show(173335, 13), parts=parts, listed=["0", "1"])
        self.assertEqual(predict(later)["why"], "conflict")
        later["listed"] = ["1", "2"]
        self.assertEqual(predict(later)["shape"], "split_parts")

    def test_a_later_entry_never_takes_its_prequel_s_extras(self):
        s1, s2 = kitsu(1, 12, "2020-01-05", ""), kitsu(2, 12, "2020-07-05", "")
        listed = ["0", "1", "2"]
        through = evidence(s2, show(500, 13), listed=listed, sharing=[s1], parts=[s1, s2])
        self.assertEqual(predict(through)["why"], "unexplained")
        mapped = evidence(s2, show(500, 13), listed=listed, sharing=[s1, s2], parts=[s2])
        self.assertEqual(predict(mapped)["why"], "unexplained")

    def test_distinct_fractions_are_distinct_extras(self):
        listed = ["0", "1", "6.1", "6.2", "6.2"]
        self.assertEqual(predict(evidence(kitsu(1, 12), show(2, 14), listed=listed))["why"], "unexplained")
        m = predict(evidence(kitsu(1, 12), show(2, 15), listed=listed))
        self.assertEqual(m["segments"][0]["extras_after"], [0, 6, 6])

    def test_an_unfinished_entry_is_not_completed(self):
        ev = ranma()
        ev["chain"][0]["finished"] = False
        self.assertEqual(targets(predict(ev), 161)[160], [149939, 143, False, [[210, 18, True]]])


class Generation(unittest.TestCase):
    def test_the_set_is_reproducible_and_ids_unique(self):
        a, b = oracle.gen(), oracle.gen()
        self.assertEqual(a, b)
        ids = [c["id"] for c in a]
        self.assertEqual(len(ids), len(set(ids)))

    def test_every_family_is_generated_and_every_shape_predicted(self):
        cases = oracle.gen()
        families = {c["family"].split("-")[0] for c in cases}
        for f in ("chain", "shared", "split", "extras", "conflict", "plain", "recorded", "agreeing", "borrowed"):
            self.assertIn(f, families)
        full = {c["family"] for c in cases}
        for f in ("chain-dated-both", "chain-dated-start_only", "chain-slack-1", "chain-slack-3",
                  "chain-early-dated-both", "chain-start-slack-2", "chain-middle-dated"):
            self.assertIn(f, full)
        shapes = {c["expect"]["map"]["shape"] for c in cases}
        self.assertEqual(shapes, {"identity", "chain", "shared_entry", "split_parts", "extras"})
        self.assertTrue(any(c["family"] == "extras-multi-ok" for c in cases))


class Comparison(unittest.TestCase):
    def setUp(self):
        self.cases = [{"id": "a", "family": "plain", "expect": {"map": oracle.identity("counts_agree"), "targets": [None]}}]

    def test_agreement_is_no_problem(self):
        problems, _ = oracle.cmp(self.cases, [{"id": "a", "got": copy.deepcopy(self.cases[0]["expect"])}], [])
        self.assertEqual(problems, [])

    def test_a_disagreement_a_missing_outcome_and_bad_allowlist_entries_are_problems(self):
        wrong = {"map": oracle.identity("unexplained"), "targets": [None]}
        problems, _ = oracle.cmp(self.cases, [{"id": "a", "got": wrong}], [])
        self.assertEqual(len(problems), 1)
        problems, _ = oracle.cmp(self.cases, [], [])
        self.assertIn("a: no outcome", problems)
        problems, _ = oracle.cmp(self.cases, [{"id": "a", "got": wrong}], [{"id": "a", "got": wrong}])
        self.assertEqual(problems, ["a: allowlisted without a reason"])
        problems, _ = oracle.cmp(self.cases, [{"id": "a", "got": wrong}], [{"id": "zz", "got": wrong, "reason": "r"}])
        self.assertTrue(any("no such case" in p for p in problems))
        right = copy.deepcopy(self.cases[0]["expect"])
        problems, _ = oracle.cmp(self.cases, [{"id": "a", "got": right}], [{"id": "a", "got": wrong, "reason": "r"}])
        self.assertTrue(any("agrees now" in p for p in problems))

    def test_an_allowlisted_outcome_that_moves_again_is_a_problem(self):
        wrong = {"map": oracle.identity("unexplained"), "targets": [None]}
        other = {"map": oracle.identity("conflict"), "targets": [None]}
        problems, _ = oracle.cmp(self.cases, [{"id": "a", "got": other}], [{"id": "a", "got": wrong, "reason": "r"}])
        self.assertEqual(len(problems), 1)


if __name__ == "__main__":
    unittest.main()
