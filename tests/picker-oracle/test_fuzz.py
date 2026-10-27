#!/usr/bin/env python3
"""The differential fuzzer's own tests: that a seed decides its pools,
that every pool it writes is one the model speaks for, and that its
command line writes what `cmp` reads.

    python3 -I -m unittest discover -s tests/picker-oracle -p 'test_*.py'
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import fuzz  # noqa: E402
import oracle  # noqa: E402

N = 300


class Fuzz(unittest.TestCase):
    def test_a_seed_decides_its_pools(self):
        self.assertEqual(fuzz.fuzz(7, N), fuzz.fuzz(7, N))
        self.assertNotEqual(fuzz.fuzz(7, N), fuzz.fuzz(8, N))

    def test_a_longer_run_extends_a_shorter_one(self):
        # the budget only decides where the draw stops, so a failing pool
        # found at a large budget is found again at the same seed
        self.assertEqual(fuzz.fuzz(7, N * 2)[:N], fuzz.fuzz(7, N))

    def test_every_pool_carries_the_model_s_prediction(self):
        for case in fuzz.fuzz(11, N):
            want = oracle.predict(case)
            self.assertIsNot(want, oracle.NOT_MODELLED, case["id"])
            self.assertEqual(case["expect"], want, case["id"])
            self.assertEqual(case["family"], "fuzz")

    def test_every_pool_is_a_search_the_entry_s_titles_make(self):
        for case in fuzz.fuzz(13, N):
            self.assertTrue(case["titles"])
            self.assertEqual(len(set(case["titles"])), len(case["titles"]))
            self.assertTrue(all(t.strip() for t in case["titles"]))
            self.assertTrue(case["rows"])
            self.assertEqual(len({r[0] for r in case["rows"]}), len(case["rows"]))

    def test_ids_are_unique_and_follow_the_pool(self):
        cases = fuzz.fuzz(17, N)
        self.assertEqual(len({c["id"] for c in cases}), len(cases))
        for c in cases:
            names = {k: c[k] for k in ("jnames", "page_names") if k in c} or None
            self.assertEqual(
                c["id"], oracle.case_id("fuzz", c["titles"], c["count"], c["entry_year"], c["rows"], (), names)
            )

    def test_the_grammar_reaches_every_part_of_the_vocabulary(self):
        # names in every script, numbers in either width, Roman numerals,
        # spelled ordinals, the division words, tags in every bracket pair,
        # punctuation, and joins with no space
        text = "\n".join(t for c in fuzz.fuzz(19, 3000) for t in c["titles"] + [r[1] for r in c["rows"]])
        for piece in ["ショー", "Café", "ＳＨＯＷ", "２", "II", "Second", "Season", "Part", "Cour", "Stage",
                      "Half", "Final Season", "第", "(", "［", "（", "【", "Uncensored", "!", "☆", "・"]:
            self.assertIn(piece, text, piece)
        self.assertTrue(any(c["count"] is None for c in fuzz.fuzz(19, 300)))
        self.assertTrue(any(c["entry_year"] is None for c in fuzz.fuzz(19, 300)))

    def test_the_command_line_writes_the_pools(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "f.json")
            run = subprocess.run(
                [sys.executable, "-I", os.path.join(HERE, "oracle.py"), "fuzz", "5", "40", out],
                capture_output=True,
                text=True,
                check=True,
            )
            self.assertIn("40 pools from seed 5", run.stdout)
            with open(out, encoding="utf-8") as fh:
                self.assertEqual(json.load(fh), fuzz.fuzz(5, 40))


if __name__ == "__main__":
    unittest.main()
