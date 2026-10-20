"""A model of the episode-map rules in docs/title-resolution.md
("Kitsu's episodes on a tracker's entries"), and the comparison of the
app's maps with it.

    python3 -I tests/tracker-map-oracle/oracle.py gen CASES.json
    python3 -I tests/tracker-map-oracle/oracle.py cmp CASES.json OUT.json \
        --allowlist tests/tracker-map-oracle/allowlist.json

`gen` writes every case the model knows the docs' answer for — the
synthetic shapes of every family and every recorded case under
tests/fixtures/tracker-map/ — each with the evidence and the map and
per-episode writes the model predicts. The Rust driver
(`run_the_tracker_map_oracle_cases`) maps every case and writes what
the app answered; `cmp` compares the two. Standard library only, and
nothing here reads the app's source.
"""

import hashlib
import json
import os
import random
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))

# --- the model ---------------------------------------------------------


def identity(why):
    return {"shape": "identity", "why": why, "segments": []}


def month(date):
    """`YYYY-MM` of a date, None unless the date is a real calendar date
    (`YYYY-MM-DD`) or month (`YYYY-MM`)."""
    import datetime

    if not date or not re.fullmatch(r"\d{4}-\d{2}(-\d{2})?", date):
        return None
    try:
        datetime.date(int(date[:4]), int(date[5:7]), int(date[8:]) if len(date) == 10 else 1)
    except ValueError:
        return None
    return date[:7]


def segment(entry, total, from_, to, lead=0, extras=()):
    return {
        "entry": entry["id"],
        "entry_total": total,
        "entry_finished": entry["finished"],
        "kitsu_from": from_,
        "kitsu_to": to,
        "lead": lead,
        "extras_after": list(extras),
    }


def chain_rule(ev, count, total):
    """One Kitsu entry spans the tracker's chain: the entry and its single
    sequels sum exactly to Kitsu's count, every run inside Kitsu's run by
    month, no later segment a Kitsu entry's own."""
    segs = [segment(ev["entry"], total, 1, total)]
    shows = [ev["entry"]]
    covered = total
    for hop in ev["chain"]:
        if covered >= count:
            break
        n = hop["episodes"]
        if not n:
            return None
        segs.append(segment(hop, n, covered + 1, covered + n))
        shows.append(hop)
        covered += n
    if covered != count or len(segs) < 2:
        return None
    if any(s["entry"] in ev["claimed"] for s in segs[1:]):
        return None
    ks, ke = month(ev["kitsu"]["start"]), month(ev["kitsu"]["end"])
    if not ks or not ke:
        return None
    for show in shows:
        s, e = month(show["start"]), month(show["end"])
        if not s or not e or s < ks or e > ke:
            return None
    return segs


def place(parts, ours, total):
    """The episodes before `ours` and its count, when the parts up to some
    one of them sum exactly to `total`, at least two, `ours` among them."""
    covered, found, taken = 0, None, 0
    for p in parts:
        if covered >= total:
            break
        if not p["episodes"]:
            return None
        if p["id"] == ours:
            found = (covered, p["episodes"])
        covered += p["episodes"]
        taken += 1
    if covered == total and taken >= 2:
        return found
    return None


def shared_rule(ev, total):
    parts = ev["sharing"]
    if any(not (p["start"] and len(p["start"]) == 10 and month(p["start"])) for p in parts):
        return None
    parts = sorted(parts, key=lambda p: p["start"])
    starts = [p["start"] for p in parts]
    if len(set(starts)) != len(starts):
        return None
    got = place(parts, ev["kitsu"]["id"], total)
    if got is None:
        return None
    lead, n = got
    return [segment(ev["entry"], total, 1, n, lead)]


def split_rule(ev, total):
    got = place(ev["parts"], ev["kitsu"]["id"], total)
    if got is None:
        return None
    lead, n = got
    return [segment(ev["entry"], total, 1, n, lead)]


def listed_number(text):
    """A listed number as (whole, fraction digits or None)."""
    m = re.fullmatch(r"(\d+)(?:\.(\d+))?", text)
    if not m:
        return None
    fraction = (m.group(2) or "").rstrip("0") or None
    return int(m.group(1)), fraction


def extras_rule(ev, count, total):
    if ev["listed"] is None:
        return None
    extras = sorted(
        {n for n in map(listed_number, ev["listed"]) if n and (n[0] == 0 or n[1] is not None)}
    )
    if len(extras) != total - count:
        return None
    if any(w > count for w, _ in extras):
        return None
    return [segment(ev["entry"], total, 1, count, 0, [w for w, _ in sorted(extras, key=lambda e: (e[0], e[1] or ""))])]


def predict(ev):
    count = ev["kitsu"]["episodes"] or 0
    total = ev["entry"]["episodes"] or 0
    if not count or not total:
        return identity("count_unknown")
    if count == total:
        return identity("counts_agree")
    found = []
    if total < count:
        c = chain_rule(ev, count, total)
        if c:
            found.append(("chain", c))
    else:
        ours = ev["kitsu"]["id"]
        # The list explains a surplus over the show's own count only where
        # no other Kitsu entry is filed under the entry; beside Kitsu parts
        # it is a rival to them, read from the first part's count.
        sole = all(k["id"] == ours for k in ev["sharing"] + ev["parts"])
        split = split_rule(ev, total)
        rival = None
        if split and ev["parts"]:
            first = ev["parts"][0]["episodes"]
            if first and first < total:
                rival = extras_rule(ev, first, total)
        for shape, segs in (
            ("shared_entry", shared_rule(ev, total)),
            ("split_parts", split),
            ("extras", rival),
            ("extras", extras_rule(ev, count, total) if sole else None),
        ):
            if segs and not any(segs == f[1] for f in found):
                found.append((shape, segs))
    if not found:
        return identity("unexplained")
    if len(found) > 1:
        return identity("conflict")
    return {"shape": found[0][0], "segments": found[0][1]}


def targets(m, count):
    """Every Kitsu episode's write under map `m`, 1..count+1: `None` for
    identity, else [entry, progress, complete, [[earlier entry, total,
    finished], …]]."""
    out = []
    for k in range(1, count + 2):
        hit = None
        for i, s in enumerate(m["segments"]):
            if s["kitsu_from"] <= k <= s["kitsu_to"]:
                hit = (i, s)
                break
        if hit is None:
            out.append(None)
            continue
        i, s = hit
        before = sum(1 for e in s["extras_after"] if e < k)
        position = s["lead"] + (k - s["kitsu_from"] + 1) + before
        ends = (
            k == s["kitsu_to"]
            and s["lead"] + (s["kitsu_to"] - s["kitsu_from"] + 1) + len(s["extras_after"])
            == s["entry_total"]
        )
        progress = s["entry_total"] if ends else position
        earlier = [[e["entry"], e["entry_total"], e["entry_finished"]] for e in m["segments"][:i]]
        out.append([s["entry"], progress, ends and s["entry_finished"], earlier])
    return out


# --- the cases ---------------------------------------------------------

TARGET_CAP = 300


def show(i, n, finished=True, start="2010-01-05", end="2010-12-20", id_mal=None):
    return {"id": i, "id_mal": id_mal, "episodes": n, "finished": finished, "start": start, "end": end}


def kitsu(i, n, start="2010-01-05", end="2010-12-20"):
    return {"id": str(i), "episodes": n, "start": start, "end": end}


def evidence(k, entry, **kw):
    ev = {"kitsu": k, "entry": entry, "chain": [], "claimed": [], "listed": None, "sharing": [], "parts": []}
    ev.update(kw)
    return ev


def case_id(family, ev):
    h = hashlib.sha256(json.dumps(ev, sort_keys=True).encode()).hexdigest()[:10]
    return f"{family}/{h}"


def months(base_year, i):
    y = base_year + (i // 12)
    return f"{y:04d}-{(i % 12) + 1:02d}"


def chain_family(rng):
    out = []
    for _ in range(400):
        n = rng.randint(2, 5)
        counts = [rng.randint(1, 60) for _ in range(n)]
        fins = [rng.random() < 0.85 for _ in range(n)]
        total = sum(counts)
        segs = [show(100 + i, counts[i], fins[i], months(2001, 3 * i) + "-02", months(2001, 3 * i + 2) + "-20") for i in range(n)]
        k = kitsu(1, total, "2001-01-01", months(2001, 3 * n) + "-28")
        tweak = rng.choice(["ok", "ok", "ok", "over", "under", "claimed", "early", "late", "undated", "unknown_hop", "year_only", "extra_hop", "impossible"])
        ev = evidence(k, segs[0], chain=segs[1:])
        if tweak == "over":
            ev["kitsu"]["episodes"] = total + 1
        elif tweak == "under":
            ev["kitsu"]["episodes"] = max(counts[0] + 1, total - 1)
        elif tweak == "claimed":
            ev["claimed"] = [segs[rng.randint(1, n - 1)]["id"]]
        elif tweak == "early":
            ev["entry"] = dict(ev["entry"], start="2000-11-01")
        elif tweak == "late":
            ev["chain"][-1] = dict(ev["chain"][-1], end="2030-01-01")
        elif tweak == "undated":
            ev["kitsu"]["end"] = None
        elif tweak == "unknown_hop":
            ev["chain"][0] = dict(ev["chain"][0], episodes=None)
        elif tweak == "impossible":
            ev["entry"] = dict(ev["entry"], start=rng.choice(["2001-03-31x", "2001-02-30", "2001-13-05", "2001-00"]))
        elif tweak == "year_only":
            ev["chain"][-1] = dict(ev["chain"][-1], start=ev["chain"][-1]["start"][:4])
        elif tweak == "extra_hop":
            ev["chain"].append(show(999, 6, True, "2030-01-01", "2030-02-01"))
        out.append(("chain-" + tweak, ev))
    return out


def shared_family(rng):
    out = []
    for _ in range(300):
        n = rng.randint(2, 4)
        counts = [rng.randint(1, 40) for _ in range(n)]
        parts = [kitsu(10 + i, counts[i], months(2015, 4 * i) + "-03", months(2015, 4 * i + 3) + "-25") for i in range(n)]
        ours = rng.randrange(n)
        tweak = rng.choice(["ok", "ok", "ok", "short", "same_day", "undated", "not_ours", "shuffled"])
        sharing = list(parts)
        total = sum(counts)
        if tweak == "short":
            total += 1
        elif tweak == "same_day" and n > 1:
            sharing[1] = dict(sharing[1], start=sharing[0]["start"])
        elif tweak == "undated":
            sharing[0] = dict(sharing[0], start=None)
        elif tweak == "not_ours":
            ours = None
        elif tweak == "shuffled":
            rng.shuffle(sharing)
        k = parts[ours] if ours is not None else kitsu(99, 3)
        out.append(("shared-" + tweak, evidence(k, show(7, total, rng.random() < 0.85), sharing=sharing)))
    return out


def split_family(rng):
    out = []
    for _ in range(300):
        n = rng.randint(2, 4)
        counts = [rng.randint(1, 60) for _ in range(n)]
        parts = [kitsu(20 + i, counts[i], months(2018, 3 * i) + "-01", "") for i in range(n)]
        ours = rng.randrange(n)
        total = sum(counts)
        tweak = rng.choice(["ok", "ok", "ok", "past_sum", "short", "unknown_part", "alone"])
        if tweak == "past_sum":
            parts.append(kitsu(50, 12, "2030-01-01", ""))
            if rng.random() < 0.5:
                ours = len(parts) - 1
        elif tweak == "short":
            total -= 1
        elif tweak == "unknown_part":
            parts[-1] = dict(parts[-1], episodes=None)
        elif tweak == "alone":
            parts = parts[:1]
            ours = 0
        ev = evidence(parts[ours], show(9, total, rng.random() < 0.85), parts=parts)
        if rng.random() < 0.2 and counts[0] < total:
            # The entry's list names extras for the surplus over the
            # first part: a later part conflicts.
            ev["listed"] = ["0"] + [f"{i}.5" for i in range(1, total - counts[0])]
            tweak += "-listed"
        out.append(("split-" + tweak, ev))
    return out


def later_part_family(rng):
    """A later Kitsu part with no entry of its own, filed through its
    prequels: the parts under the entry run from the first of them, three
    to six long, the earlier ones all mapped to the entry and so beside
    it as entries sharing it."""
    out = []
    for _ in range(120):
        n = rng.randint(3, 6)
        counts = [rng.randint(1, 30) for _ in range(n)]
        parts = [kitsu(80 + i, counts[i], months(2016, 4 * i) + "-02", "") for i in range(n)]
        total = sum(counts)
        tweak = rng.choice(["ok", "ok", "ok", "short", "from_nearest"])
        if tweak == "short":
            total += 1
        elif tweak == "from_nearest":
            # The parts as a walk that stopped at the nearest prequel
            # would gather them: the earlier ones left out.
            parts = parts[-2:]
        ev = evidence(parts[-1], show(88, total, rng.random() < 0.85), parts=parts, sharing=parts[:-1])
        out.append(("later_part-" + tweak, ev))
    return out


def extras_family(rng):
    """A prologue, several half-episode recaps, both, as a long shounen's
    tracker counts them — and the lists that leave the surplus
    unexplained."""
    out = []
    for _ in range(500):
        count = rng.choice([12, 13, 24, 25, 26, 50, 51, 64, 148, 220])
        after = sorted(rng.sample(range(0, count + 1), rng.randint(1, 6)))
        listed = [str(n) for n in range(1, count + 1)]
        for e in after:
            listed.append("0" if e == 0 else f"{e}.5")
            if rng.random() < 0.3:
                listed.append("0" if e == 0 else f"{e}.5")
        rng.shuffle(listed)
        total = count + len(after)
        tweak = rng.choice(["ok", "ok", "ok", "ok", "one_short", "one_over", "unlisted", "junk", "past_count"])
        if tweak == "one_short":
            total += 1
        elif tweak == "one_over":
            total -= 1
            if total <= count:
                total = count + 1
                listed += ["3.5", "4.5"]
        elif tweak == "unlisted":
            listed = None
        elif tweak == "junk":
            listed += ["Prologue", "", "2.x", "Special"]
        elif tweak == "past_count":
            listed.append(f"{count + 3}.5")
            total += 1
        family = "extras-multi-" + tweak if len(after) > 1 else "extras-" + tweak
        out.append((family, evidence(kitsu(30, count), show(31, total, rng.random() < 0.85), listed=listed)))
    return out


def conflict_family(rng):
    out = []
    for _ in range(100):
        count = rng.randint(2, 30)
        part = rng.randint(1, 4)
        listed = ["0"] + [f"{i}.5" for i in range(1, part)]
        parts = [kitsu(40, count, "2020-01-01", ""), kitsu(41, part, "2020-06-01", "")]
        same = rng.random() < 0.3
        if same:
            # Both rules file it the same way: one shared mapping and the
            # same chain.
            ev = evidence(parts[0], show(42, count + part), parts=parts, sharing=parts)
            out.append(("agreeing-rules", ev))
        else:
            ev = evidence(parts[0], show(42, count + part), parts=parts, listed=listed)
            out.append(("conflict", ev))
    return out


def borrowed_family(rng):
    """A later Kitsu entry reaching an entry whose list explains a
    surplus over the first entry's count, filed through it or mapped to
    it beside the first: never the extras rule's case."""
    out = []
    for _ in range(100):
        first, ours = rng.randint(2, 26), rng.randint(2, 26)
        listed = ["0"] + [str(i) for i in range(1, first + 1)]
        s1, s2 = kitsu(70, first, "2020-01-05", ""), kitsu(71, ours, "2020-07-05", "")
        shape = rng.choice(["through", "mapped"])
        if shape == "through":
            ev = evidence(s2, show(72, first + 1), listed=listed, sharing=[s1], parts=[s1, s2])
        else:
            ev = evidence(s2, show(72, first + 1), listed=listed, sharing=[s1, s2], parts=[s2])
        out.append(("borrowed-" + shape, ev))
    for _ in range(60):
        count = rng.randint(8, 30)
        after = rng.randint(1, count - 1)
        fracs = rng.sample(["1", "2", "3", "5"], rng.randint(2, 3))
        listed = [str(i) for i in range(1, count + 1)] + [f"{after}.{f}" for f in fracs]
        total = count + len(fracs) + rng.choice([0, 0, -1])
        out.append(("extras-fractions", evidence(kitsu(73, count), show(74, total), listed=listed)))
    return out


def plain_family(rng):
    out = []
    for _ in range(100):
        n = rng.randint(1, 300)
        t = rng.choice([n, None, 0, n + rng.randint(1, 40), max(1, n - rng.randint(1, 40))])
        k = kitsu(60, rng.choice([n, n, None]))
        out.append(("plain", evidence(k, show(61, t))))
    return out


def recorded():
    out = []
    for tracker in ("anilist", "mal"):
        path = os.path.join(REPO, "tests/fixtures/tracker-map", tracker + ".json")
        with open(path, encoding="utf-8") as f:
            for c in json.load(f)["cases"]:
                out.append(("recorded/" + c["id"], c["evidence"]))
    return out


def gen():
    rng = random.Random(20261018)
    raw = []
    for family in (chain_family, shared_family, split_family, extras_family, conflict_family, borrowed_family, plain_family, later_part_family):
        raw += family(rng)
    cases, seen = [], set()
    for family, ev in raw:
        cid = case_id(family, ev)
        if cid in seen:
            continue
        seen.add(cid)
        cases.append({"id": cid, "family": family, "evidence": ev})
    for cid, ev in recorded():
        cases.append({"id": cid, "family": "recorded", "evidence": ev})
    for c in cases:
        m = predict(c["evidence"])
        count = min(c["evidence"]["kitsu"]["episodes"] or 0, TARGET_CAP)
        c["expect"] = {"map": m, "targets": targets(m, count)}
    return cases


# --- the comparison ----------------------------------------------------


def cmp(cases, got, allow):
    by_id = {g["id"]: g["got"] for g in got}
    allowed = {a["id"]: a for a in allow}
    problems, families = [], {}
    for a in allow:
        if not a.get("reason"):
            problems.append(f"{a.get('id')}: allowlisted without a reason")
        if a.get("id") not in {c["id"] for c in cases}:
            problems.append(f"{a.get('id')}: allowlisted but no such case")
    for c in cases:
        fam = families.setdefault(c["family"].split("/")[0], [0, 0])
        fam[0] += 1
        g = by_id.get(c["id"])
        if g is None:
            problems.append(f"{c['id']}: no outcome")
            continue
        if g == c["expect"]:
            if c["id"] in allowed:
                problems.append(f"{c['id']}: allowlisted but agrees now")
            continue
        a = allowed.get(c["id"])
        if a and a.get("got") == g:
            continue
        fam[1] += 1
        problems.append(f"{c['id']}: model {json.dumps(c['expect']['map'])} app {json.dumps(g.get('map'))}"
                        + ("" if g.get("map") != c["expect"]["map"] else " (targets differ)"))
    return problems, families


def main(argv):
    if len(argv) >= 2 and argv[0] == "gen":
        with open(argv[1], "w", encoding="utf-8") as f:
            json.dump(gen(), f)
        return 0
    if len(argv) >= 3 and argv[0] == "cmp":
        allow = []
        if "--allowlist" in argv:
            with open(argv[argv.index("--allowlist") + 1], encoding="utf-8") as f:
                allow = json.load(f)
        with open(argv[1], encoding="utf-8") as f:
            cases = json.load(f)
        with open(argv[2], encoding="utf-8") as f:
            got = json.load(f)
        problems, families = cmp(cases, got, allow)
        print(f"{len(cases)} cases, {len(problems)} problems")
        for fam, (n, bad) in sorted(families.items()):
            print(f"  {fam}: {n} cases, {bad} disagree")
        for p in problems:
            print(p)
        return 1 if problems else 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
