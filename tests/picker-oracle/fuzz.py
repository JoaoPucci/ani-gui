"""The differential fuzzer: pools built from a grammar over the docs'
vocabulary, each carrying the pick the model predicts for it.

    oracle.py fuzz SEED N CASES.json

writes N pools drawn from the seed — the same seed and N write the same
pools, byte for byte — in the shape `gen` writes, for the same driver
and the same `cmp`. Where `gen` walks a fixed lattice of shapes the
docs' tables name, this draws titles from the words those tables are
written in and joins them every way a catalogue writes them: names in
Latin, accented and Japanese script, numbers in ASCII and full-width
digits, Roman numerals and spelled ordinals, the division words (Season,
Part, Cour, Stage, a spelled ordinal before one, a Half, a Final
Season), catalogue and version tags in every bracket pair and in either
width, punctuation, and words glued with no space between them. The
listings are built from the entry's own name, so they are the shapes a
search for it answers with.

A pool the model does not compute — `predict` answering `NOT_MODELLED`:
a broad listing a sibling could complete, parts that could stitch, a
one-video entry — is drawn again, so every pool written is one the
model speaks for.
"""

import random

import oracle

#: Names a show goes by, as the docs' examples write them: plain, with a
#: number of its own, with marks the comparison drops or keeps, accented,
#: in full-width Latin, in Japanese.
NAMES = [
    "Show",
    "Show Other",
    "Other Show",
    "Lucky",
    "Café",
    "Pokémon",
    "ＳＨＯＷ",
    "Mob Psycho 100",
    "Kaiju No. 8",
    "86",
    "Steins;Gate",
    "Gintama",
    "Haikyuu!!",
    "Spy x Family",
    "The Seventh Son",
    "Show: Arc",
    "ショー",
    "怪獣８号",
    "推しの子",
]

#: Names another show goes by: what a search answers with beside the
#: entry's own listings.
OTHERS = ["Other", "Zeta Thing", "Showtime", "Show Side Story", "Other Name"]

ROMAN = ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX"]
SPELLED = ["First", "Second", "Third", "Fourth", "Seventh", "Tenth"]
CARDINAL = ["One", "Two", "Three", "Seven"]
KANJI = ["一", "二", "三", "十", "十二", "〇"]
MARKERS = ["Season", "Part", "Cour", "Stage"]
SUFFIX = {1: "st", 2: "nd", 3: "rd"}

#: Catalogue tags and version tags, each in every bracket pair and in
#: either width, a character at a time.
CTAG_WORDS = ["TV", "ONA", "OVA", "OAD", "2020", "2011"]
VTAG_WORDS = ["Uncensored", "UNCENSORED", "uncensored"]
BRACKETS = [("(", ")"), ("[", "]"), ("（", "）"), ("［", "］"), ("【", "】")]

#: What joins two pieces of a title: a space, nothing, or a separator.
JOINS = [" ", " ", " ", "", ": ", " - ", ", ", "・"]

#: Marks a catalogue writes at a word's end or a title's end.
MARKS = ["!", "?", "!!", ".", "☆", "'", "~", "♪"]


def _wide(c):
    return chr(ord(c) + 0xFEE0) if "!" <= c <= "~" else c


def _width(rng, text):
    """`text` in ASCII, full-width, or mixed a character at a time."""
    how = rng.random()
    if how < 0.6:
        return text
    if how < 0.8:
        return "".join(_wide(c) for c in text)
    return "".join(_wide(c) if rng.random() < 0.5 else c for c in text)


def _number(rng, k):
    return _width(rng, str(k))


def _ordinal(rng):
    return rng.choice([0, 1, 1, 2, 2, 2, 3, 3, 4, 7, 10, 12, 20])


def division(rng, k=None):
    """One division naming the ordinal `k`, written one of the ways the
    docs' grammar reads one — or a word the grammar must not read as one."""
    if k is None:
        k = _ordinal(rng)
    n = _number(rng, k)
    shapes = [
        lambda: f"{rng.choice(MARKERS)} {n}",
        lambda: f"{n}{SUFFIX.get(k % 10 if k not in (11, 12, 13) else 0, 'th')} {rng.choice(MARKERS)}",
        lambda: f"{SPELLED[min(k, 6) - 1] if 1 <= k <= 6 else 'Seventh'} {rng.choice(MARKERS + ['Half'])}",
        lambda: f"Season {CARDINAL[min(max(k, 1), 4) - 1]}",
        lambda: ROMAN[min(max(k, 2), 9) - 2],
        lambda: n,
        lambda: f"第{n}{rng.choice(['期', '部', 'クール'])}",
        lambda: f"第{rng.choice(KANJI)}{rng.choice(['期', '部'])}",
        lambda: rng.choice(SPELLED),
        lambda: rng.choice(["Final Season", "Second Half", "First Half", "Final Arc", "Recap"]),
        lambda: f"Part {n}+{_number(rng, k + 1)}",
        lambda: f"{n}{SUFFIX.get(k, 'th')}",
    ]
    return rng.choice(shapes)()


def tag(rng):
    """A catalogue tag or a version tag, in some bracket pair and width,
    sometimes with a space inside."""
    o, c = rng.choice(BRACKETS)
    word = rng.choice(CTAG_WORDS + VTAG_WORDS if rng.random() < 0.7 else VTAG_WORDS)
    inner = _width(rng, word)
    if rng.random() < 0.1:
        inner = f" {inner} "
    return f"{o}{inner}{c}"


def decorate(rng, title):
    """Marks and tags where catalogues put them: at the end, after the
    name, glued or spaced."""
    roll = rng.random()
    if roll < 0.15:
        title += rng.choice(MARKS)
    elif roll < 0.35:
        title += rng.choice([" ", "", " "]) + tag(rng)
    elif roll < 0.42:
        words = title.split(" ")
        if len(words) > 1:
            i = rng.randrange(1, len(words))
            words.insert(i, tag(rng))
            title = " ".join(words)
    return title


def title_of(rng, name, divisions):
    """A name and the divisions after it, joined the ways catalogues join
    them."""
    out = name
    for d in divisions:
        out += rng.choice(JOINS) + d
    return decorate(rng, out)


def entry_titles(rng, name):
    """The entry's canonical title, and sometimes an alias: the same name
    under another division, another script, or a name of its own."""
    divs = [division(rng) for _ in range(rng.choice([0, 0, 1, 1, 1, 2]))]
    titles = [title_of(rng, name, divs)]
    roll = rng.random()
    if roll < 0.15:
        titles.append(title_of(rng, name, [division(rng) for _ in range(rng.choice([0, 1]))]))
    elif roll < 0.22:
        titles.insert(0, "Zzyzx Alias")
    elif roll < 0.28:
        titles.append("ショウ ゼータ")
    # whitespace-only and duplicate titles are no search; keep them out
    seen, out = set(), []
    for t in titles:
        if t.strip() and t not in seen:
            seen.add(t)
            out.append(t)
    return out, divs


def candidate(rng, name, divs):
    """A listing a search for the entry answers with: the entry's own
    name under its divisions, under others, under a variant of the
    name, or another show."""
    roll = rng.random()
    if roll < 0.3:
        d = list(divs)
    elif roll < 0.6:
        d = [division(rng) for _ in range(rng.choice([0, 1, 1, 2]))]
    elif roll < 0.75:
        d = list(divs) + [division(rng)]
    elif roll < 0.85:
        d = divs[:-1]
    else:
        return decorate(rng, rng.choice(OTHERS) + rng.choice(["", " " + division(rng)]))
    nm = name
    if rng.random() < 0.1:
        nm = rng.choice([name + rng.choice(MARKS), _width(rng, name), name.split(" ")[0]])
    return title_of(rng, nm, d)


COUNTS = [oracle.E] * 5 + [oracle.E - 1, oracle.E + 1, 2 * oracle.E, 40, 5]


def pool(rng):
    """One pool: the entry, its count and year, and what its searches
    answer with. Card and page names follow the driver's convention
    unless the pool gives its own."""
    name = rng.choice(NAMES)
    titles, divs = entry_titles(rng, name)
    count = rng.choice([oracle.E, oracle.E, oracle.E, None])
    year = rng.choice([oracle.LISTING_YEAR, oracle.LISTING_YEAR, None])
    rows = []
    for i in range(rng.choice([1, 1, 2, 2, 3, 4])):
        rows.append([f"s{i}", candidate(rng, name, divs), rng.choice(COUNTS)])
    names = None
    if count is not None and year is not None and rng.random() < 0.2:
        names = {"jnames": {}}
    return titles, count, year, rows, names


def fuzz(seed, n):
    """`n` pools drawn from `seed`, each with the model's prediction."""
    rng = random.Random(seed)
    out, ids = [], set()
    while len(out) < n:
        titles, count, year, rows, names = pool(rng)
        case = dict(titles=titles, count=count, entry_year=year, rows=rows, **(names or {}))
        expect = oracle.predict(case)
        if expect is oracle.NOT_MODELLED:
            continue
        cid = oracle.case_id("fuzz", titles, count, year, rows, (), names)
        if cid in ids:
            continue
        ids.add(cid)
        case.update(id=cid, expect=expect, family="fuzz")
        out.append(case)
    return out
