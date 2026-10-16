#!/usr/bin/env python3
"""A model of the season picker, read from docs/title-resolution.md.

The predicates here are transcriptions of the document's text — the
title rule, the entry before and the season before, "One reading of a
title", the probe head, the countless order — and the pools whose
answers those predicates do not compute (the spanning cut, dead
listings, the stitching of parts) are tabled in `gen()` row by row from
the decision tables. Nothing here is read from the code: when the
model and the picker disagree, one of them is wrong about the docs, and
the comparison says which pool.

    oracle.py gen CASES.json
        writes every pool with the pick the docs answer for it — the
        generated families and the hand-written sets under cases/
    oracle.py cmp CASES.json OUT.json [--allowlist allowlist.json]
        reads the driver's outcomes for those pools and exits non-zero
        on a disagreement the allowlist does not carry, an allowlisted
        one whose outcome moved, or an allowlist entry gone stale

Standard library only. Python 3.12 is what the CI runners carry.
"""

import hashlib
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

#: The count every generated pool is built around.
E = 12

#: The year the driver writes on every listing's entry page, and the
#: year a generated entry carries when it carries one — so with a year
#: every hit's year is the entry's, and, with the entry's name on every
#: card unless a pool says otherwise (`card_names`), the titles alone
#: decide. The driver's `LISTING_YEAR` is this number; the two move
#: together.
LISTING_YEAR = 2020

#: What `predict` answers for a pool the predicates do not read: a
#: broad listing (the spanning cut), a bare title beside parts of it
#: whose counts could together explain the entry's (stitching), a dead
#: listing, a one-video entry. Those pools are tabled, not computed.
NOT_MODELLED = object()

JP = r"぀-ヿ一-鿿"


# --- catalogue tags: "a release year or a format tag in brackets ... wherever in the title it stands"

CTAG = re.compile(
    r"[\(\[（【]\s*(?:\d{4}|tv|ona|ova|oad|[０-９]{4}|ｔｖ|ｏｎａ|ｏｖａ|ｏａｄ)\s*[\)\]）】]"
)


# --- version tags: "a bracketed version tag ... in ASCII or full-width round,
# square or lenticular brackets, in any case" (Versions of one pick)

VTAG = re.compile(r"[\(\[（［【]\s*(?:uncensored|ｕｎｃｅｎｓｏｒｅｄ)\s*[\)\]）］】]")


def version_of(t):
    """A title read for its version: the title without its version tag,
    compared as written, and the version the tag names."""
    low = t.lower()
    kind = "uncensored" if VTAG.search(low) else "standard"
    return written(VTAG.sub(" ", low)), kind


_VTAG_ANY = re.compile(VTAG.pattern, re.IGNORECASE)


def _past_vtag(cand):
    return " ".join(_VTAG_ANY.sub(" ", cand).split())


def admitted_as_read(titles, cand):
    """The title rule for a probed candidate: admission read past a
    bracketed version tag, as past a catalogue tag — the tag hides no
    division before it. The probe head reads the title as written."""
    return admitted(titles, _past_vtag(cand))


def pickable_as_read(titles, cand):
    """A probed candidate the selectors choose among: `pickable`, read past
    a bracketed version tag — so "Show 1 [Uncensored]" is the entry before
    "Show 2" as "Show 1" is."""
    return pickable(titles, _past_vtag(cand))


def versions_of(winner, tied):
    """The versions of `winner` among the candidates `tied` with it on
    distance that could be picked: every one whose title is the winner's
    once the version tag is set aside and whose listing numbers the same
    episodes (here: the same count, every listing numbering 1..n under
    one badge), in the provider's order — when they are under different
    version tags, one each; none otherwise."""
    base = version_of(winner[1])[0]
    group = [r for r in tied if version_of(r[1])[0] == base and r[2] == winner[2]]
    kinds = [version_of(r[1])[1] for r in group]
    if len(group) < 2 or len(set(kinds)) != len(kinds):
        return []
    return [[r[0], k] for r, k in zip(group, kinds)]


def _ascii(s):
    """Full-width letters and digits read as ASCII."""
    return "".join(chr(ord(c) - 0xFEE0) if "！" <= c <= "～" else c for c in s)


def written(t):
    """Two titles compared as written: the text without tags, with the set
    of tags it carries, wherever they stand."""
    low = t.lower()
    tags = sorted(_ascii(m.group(0))[1:-1].strip() for m in CTAG.finditer(low))
    return " ".join(CTAG.sub(" ", low).split() + tags)


def norm(t):
    """Every reader reads a title lowercased and without its catalogue
    tags, full-width digits read as ASCII."""
    t = CTAG.sub(" ", t.lower())
    t = " ".join(t.split())
    return "".join(chr(ord(c) - 0xFF10 + 0x30) if "０" <= c <= "９" else c for c in t)


ROMAN = {"ii": 2, "iii": 3, "iv": 4, "v": 5, "vi": 6, "vii": 7, "viii": 8, "ix": 9}
WORD_ORD = {
    w: i + 1
    for i, w in enumerate(
        "first second third fourth fifth sixth seventh eighth ninth tenth eleventh twelfth "
        "thirteenth fourteenth fifteenth sixteenth seventeenth eighteenth nineteenth twentieth".split()
    )
}
CARD = {
    w: i + 1
    for i, w in enumerate(
        "one two three four five six seven eight nine ten eleven twelve thirteen fourteen "
        "fifteen sixteen seventeen eighteen nineteen twenty".split()
    )
}
KANJI_DIGIT = {"〇": 0, "零": 0, "一": 1, "二": 2, "三": 3, "四": 4, "五": 5, "六": 6, "七": 7, "八": 8, "九": 9}
KANJI_UNIT = {"十": 10, "百": 100}


def kanji_number(s):
    """A number written in kanji, compound ones included (第十二部 is 12,
    第百期 100, 第〇期 the zeroth); a run that writes no number that way
    is none."""
    if not s or any(c not in KANJI_DIGIT and c not in KANJI_UNIT for c in s):
        return None
    if all(c in KANJI_DIGIT for c in s):
        return KANJI_DIGIT[s] if len(s) == 1 else None
    total, current, seen_unit = 0, 0, 0
    for c in s:
        if c in KANJI_DIGIT:
            if current:
                return None
            current = KANJI_DIGIT[c]
        else:
            unit = KANJI_UNIT[c]
            if seen_unit and unit >= seen_unit:
                return None
            seen_unit = unit
            total += (current or 1) * unit
            current = 0
    return total + current


def _jp_ordinal(s):
    """The ordinal of a Japanese division's number: digits or kanji."""
    if s.isdigit():
        return int(s)
    return kanji_number(s)


# --- the title rule: "The entry's titles rule which candidates can be picked"

MARKER = r"(?:season|part|cour|stage)"


def _cardinals(t):
    """A word cardinal after a marker word, one to twenty, is that ordinal."""
    return re.sub(
        r"\b(" + MARKER + r")\s+(" + "|".join(CARD) + r")\b",
        lambda m: f"{m.group(1)} {CARD[m.group(2)]}",
        norm(t),
    )


def _word_ordinals(t):
    """A word ordinal before a marker word, first to twentieth, is that
    ordinal with its suffix."""
    return re.sub(
        r"\b(" + "|".join(WORD_ORD) + r")\s+(" + MARKER + r"|half)\b",
        lambda m: f"{WORD_ORD[m.group(1)]}th {m.group(2)}",
        _cardinals(t),
    )


def named(titles):
    """Ordinals the entry's titles name somewhere: in a marker, as any
    small number or numeral in it, as a Japanese 第N, as a number before a
    Japanese counter."""
    out = set()
    for t in titles:
        n = _word_ordinals(t)
        for m in re.finditer(MARKER + r"\s+(\d+)", n):
            out.add(int(m.group(1)))
        for m in re.finditer(r"\b(\d+)(?:st|nd|rd|th)\s+" + MARKER, n):
            out.add(int(m.group(1)))
        for m in re.finditer(r"第([0-9]+|[〇零一二三四五六七八九十百]+)", n):
            k = _jp_ordinal(m.group(1))
            if k is not None:
                out.add(k)
        for m in re.finditer(r"([0-9]+|[〇零一二三四五六七八九十百]+)[号期話部]", n):
            k = _jp_ordinal(m.group(1))
            if k is not None:
                out.add(k)
        for m in re.finditer(r"(?<![0-9a-z])(\d{1,2})(?![0-9a-z])", n):
            out.add(int(m.group(1)))
        for m in re.finditer(r"(?<![0-9a-z])(\d{1,2})(?:st|nd|rd|th)(?![0-9a-z])", n):
            out.add(int(m.group(1)))
        for w in re.findall(r"[a-z]+", n):
            if w in ROMAN:
                out.add(ROMAN[w])
            elif w in WORD_ORD:
                out.add(WORD_ORD[w])
    return out


TRAIL = [
    re.compile(r"\s*\b" + MARKER + r"\s+(\d+(?:\+\d+)*)$"),
    re.compile(r"\s*\b(\d+)(?:st|nd|rd|th)\s+" + MARKER + r"$"),
    re.compile(r"\s*第([0-9]+|[〇零一二三四五六七八九十百]+)(?:期|部|クール)$"),
    re.compile(r"\s+(ii|iii|iv|v|vi|vii|viii|ix)$"),
]


_BRACKETED_MARKER = re.compile(r"\s*[\(\[]\s*(" + MARKER + r"\s+\d+(?:\+\d+)*)\s*[\)\]]$")


def strip_trailing(n):
    """The markers a title ends on, innermost first, and what is left. A
    span ("Part 1+2") is every part in it; brackets around a marker
    ("(Part 1+2)") are not the marker's."""
    ords = []
    while True:
        n = _BRACKETED_MARKER.sub(r" \1", n)
        for r in TRAIL:
            m = r.search(n)
            if m and n[: m.start()].strip():
                span = []
                for g in m.group(1).split("+"):
                    k = ROMAN.get(g, None)
                    if k is None:
                        k = _jp_ordinal(g)
                    span.append(k)
                ords.append(tuple(span))
                n = n[: m.start()].rstrip(" :-")
                break
        else:
            return n, ords


def bare_number(stem):
    """A bare number of one or two digits a stem ends on — "Show 2", the
    ２ glued to "ショー２"."""
    m = re.search(r"(?:^|[^0-9a-z" + JP + r"])(\d{1,2})$", stem) or re.search(
        r"(?<=[" + JP + r"])(\d{1,2})$", stem
    )
    return int(m.group(1)) if m else None


_KANJI_RUN = re.compile(r"(第?)([〇零一二三四五六七八九十百]+)(?=[号期話部]|クール)")


def kanji_as_digits(n):
    """A kanji number where a Japanese number is expected — after 第, or
    before a counter — read as the number it is; the 一 of a word like
    一騎当千 stays a word."""
    def sub(m):
        k = kanji_number(m.group(2))
        return m.group(0) if k is None else f"{m.group(1)}{k}"

    return _KANJI_RUN.sub(sub, n)


def first_number(t):
    """A title's first number: the first run of digits in the first word
    that holds any, with the words before it spacing and punctuation
    dropped. A separate word after it never lengthens it; a kanji number
    before a counter is a number ("八十" continues "八")."""
    n = kanji_as_digits(norm(t))
    m = re.search(r"\d+", n)
    if not m:
        return None
    return (re.sub(r"[^0-9a-z" + JP + r"]", "", n[: m.start()]), m.group(0))


def continues(c, t):
    """`c` matches `t` only by carrying its first number with more digits
    after it: the words before alike, the number continued."""
    a, b = first_number(c), first_number(t)
    return bool(a and b and a[0] == b[0] and a[1] != b[1] and a[1].startswith(b[1]))


def admitted(titles, cand):
    """The title rule: a candidate ending on a season or part marker, or on
    a bare number, names an ordinal the entry names; a first never
    disqualifies; a title continuing the entry's first number is another
    show, unless another of the entry's titles carries that number after
    the same words."""
    nm = named(titles)
    stem, ords = strip_trailing(_word_ordinals(cand))
    for span in ords:
        # a first part or a span through it never disqualifies; any other
        # span needs every part in it named
        if 1 in span:
            continue
        if any(o not in nm for o in span):
            return False
    b = bare_number(stem)
    if b is not None and b != 1 and b not in nm:
        return False
    if any(continues(cand, t) for t in titles) and not any(
        first_number(cand) == first_number(t) for t in titles
    ):
        return False
    return True


# --- the entry before this one

def _clean(t):
    return re.sub(r"[^0-9a-z぀-ヿ一-鿿]+", " ", norm(t)).strip()


def _name(t):
    c = _clean(_word_ordinals(t))
    m = re.search(
        r"(\d|第|\b(?:ii|iii|iv|vi|vii|viii|ix|first|second|third|season|part|cour|stage)\b)", c
    )
    return (c[: m.start()] if m else c).strip()


FIRSTS = {"season", "part", "cour", "stage", "first", "1", "half"}

SEPS = set(":-–—~,.・、：－～，．]）)】」』")


def _tokens_end(text, nm_words):
    """The end of the shortest prefix of `text` whose cleaned words are
    `nm_words`, at a word's end (or before 第 / a glued digit)."""
    for end in range(1, len(text) + 1):
        if _clean(text[:end]).split() == nm_words:
            nxt = text[end : end + 1]
            if (
                not nxt
                or not nxt.isalnum()
                or nxt == "第"
                or (nxt.isdigit() and not nm_words[-1][-1].isdigit())
            ):
                return end
    return None


def _marks(text, end):
    out = ""
    for ch in text[end:]:
        if ch.isalnum():
            break
        if not ch.isspace() and ch not in SEPS:
            out += ch
    return out


def _untag(t):
    """A bracketed four-digit year or TV/ONA/OVA/OAD tag is neither a word
    nor a mark."""
    return " ".join(CTAG.sub(" ", norm(t)).split())


def _name_and_first_divisions(cand, t):
    """The name word by word, the marks right after it the title's own
    (separators none), then first divisions alone."""
    nm = _name(_untag(t)).split()
    if not nm:
        return False
    c, tt = _untag(cand), _untag(t)
    ce, te = _tokens_end(c, nm), _tokens_end(tt, nm)
    if ce is None or te is None or _marks(c, ce) != _marks(tt, te):
        return False
    toks = _clean(c[ce:]).split()
    return all(w in FIRSTS or re.fullmatch(r"1(st|nd|rd|th)|第[1１一][期部]", w) for w in toks)


def before_entry(titles, cand):
    """When every title of the entry names a later division, a candidate
    that is one of those titles' names and nothing more, or that name
    followed by first divisions alone, is the entry before it — unless it
    is one of the entry's titles or carries the number one of them does."""
    def reads(t):
        return bool(named([t]) - {1})

    return (
        not reads(cand)
        and all(reads(t) for t in titles)
        and any(_name_and_first_divisions(cand, t) for t in titles)
        and not any(
            written(t) == written(cand)
            or (first_number(cand) and first_number(cand) == first_number(t))
            for t in titles
        )
    )


def _seasons(text):
    """Seasons a text names: marker pairs, 第N期, a Roman numeral or a
    spelled ordinal standing alone."""
    t = _word_ordinals(text)
    out = [int(m) for m in re.findall(r"season\s+(\d+)", t)]
    out += [int(m) for m in re.findall(r"(\d+)(?:st|nd|rd|th)\s+season", t)]
    out += [
        k
        for k in (_jp_ordinal(m) for m in re.findall(r"第([0-9]+|[〇零一二三四五六七八九十百]+)期", t))
        if k is not None
    ]
    out += [ROMAN[m] for m in re.findall(r"\b(ii|iii|iv|v|vi|vii|viii|ix)\b", t)]
    # an ordinal standing alone after a part stays as it is read: "Show
    # Season 3 Part 1 Second Half" names a season 2, before the 3
    out += [int(m) for m in re.findall(r"part\s+\d+\s+(\d+)(?:st|nd|rd|th)\s+half", t)]
    return out


def _past_name(cand, t):
    nm = _name(t).split()
    c = _clean(cand).split()
    if not nm or c[: len(nm)] != nm:
        cc, nn = _clean(cand), _name(t)
        return cc[len(nn) :] if nn and cc.startswith(nn) else None
    return " ".join(c[len(nm) :])


def season_before_entry(titles, cand):
    """A candidate naming a season before the one an entry title carries,
    read past its stem, or whose stem ends on a lower number after the
    same words — unless it is one of the entry's titles."""
    if any(written(t) == written(cand) for t in titles):
        return False
    for t in titles:
        rest_t = _past_name(t, t)
        rest_c = _past_name(cand, t)
        if rest_t is None or rest_c is None:
            continue
        lead_t = re.match(r"^([0-9]{1,2})\b", rest_t)
        lead_c = re.match(r"^([0-9]{1,2})\b", rest_c)
        if lead_t and lead_c and int(lead_c.group(1)) < int(lead_t.group(1)):
            return True
        if lead_t and not lead_c:
            # the bare number the entry's stem ends on is the season it
            # carries for a candidate read past the stem without it
            season = max([int(lead_t.group(1))] + _seasons(rest_t[len(lead_t.group(0)) :]))
            if any(x < season for x in _seasons(rest_c)):
                return True
            continue
        same_lead = lead_t and lead_c and lead_c.group(1) == lead_t.group(1)
        ts = _seasons(rest_t[len(lead_t.group(0)) :] if same_lead else rest_t)
        season = max(ts) if ts else 1
        cs = _seasons(rest_c[len(lead_c.group(0)) :] if lead_c else rest_c)
        if any(x < season for x in cs):
            return True
    return False


_ONE_MARKER = re.compile(
    r"^(?:"
    + MARKER
    + r" (\d+)|(\d+)(?:st|nd|rd|th) "
    + MARKER
    + r"|(ii|iii|iv|v|vi|vii|viii|ix)|第([0-9]+|[〇零一二三四五六七八九十百]+)(?:期|部|クール))$"
)


def part_after(title, stem):
    """The ordinal of the one short part marker — "2nd Stage", "Part 3",
    "Season 2", "II", 第2期 — that `title` adds to `stem`, or `None` when
    `title` is not the stem followed by exactly one."""
    n = _word_ordinals(title)
    if not n.startswith(stem) or len(n) == len(stem):
        return None
    tail = n[len(stem) :]
    if tail[0].isalnum() and tail[0] != "第":
        return None
    m = _ONE_MARKER.match(tail.lstrip(" :-–—~,.・、"))
    if not m:
        return None
    g = next(x for x in m.groups() if x is not None)
    k = ROMAN.get(g)
    return _jp_ordinal(g) if k is None else k


def later_part_of(titles, cand):
    """The sequel guard: a candidate that a title the entry goes by names a
    later part of — the candidate's title followed by one short part
    marker ("Show Season 3" for "Show Season 3 Part 2", "Lucky 2" for
    "Lucky 2 Season 2") — is dropped from the pool before probing. It is
    another entry of the same show, as the entry before is. A bare number
    is no such marker: "Show" beside "Show 2" is the entry before's shape,
    with that rule's allowance for an alias naming no later division."""
    c = _word_ordinals(cand)
    return any(part_after(t, c) not in (None, 1) for t in titles)


def pickable(titles, cand):
    """A candidate the selectors choose among: admitted, and neither the
    entry before this one, the season before it, nor a title one of the
    entry's own names a later part of."""
    return (
        admitted(titles, cand)
        and not before_entry(titles, cand)
        and not season_before_entry(titles, cand)
        and not later_part_of(titles, cand)
    )


# --- "One reading of a title": the reader the entry names a hit through

_WORD = re.compile(
    r"\d+(?:\+\d+)+|[0-9a-z]+|[" + JP + r"]+(?:[0-9]+[" + JP + r"]+)*(?:[0-9]{1,2}(?![0-9]))?"
)
_JP_DIV = re.compile(r"第([0-9]+|[〇零一二三四五六七八九十百]+)(期|部|クール)")
_SPAN = re.compile(r"^(\d+)(?:\+(\d+))+$")


def _tokenize(n):
    """Words and Japanese divisions, in order. A span ("1+2") is one word.
    A Japanese run keeps the digits inside it and splits into the word,
    each 第N期 glued to it, and a one- or two-digit number glued to its
    end ("ショー第2期第2部" → ショー, 第2期, 第2部; "ショー２" → ショー, 2;
    "怪獣８号" stays one word); a longer number glued to its end is a word
    of its own, as it is written apart ("ショー200" → ショー, 200)."""
    out = []
    for m in _WORD.finditer(n):
        w = m.group(0)
        if not re.match(r"[" + JP + r"]", w):
            out.append(w)
            continue
        pos = 0
        for d in _JP_DIV.finditer(w):
            if d.start() > pos:
                out.append(w[pos : d.start()])
            out.append(d.group(0))
            pos = d.end()
        if pos < len(w):
            out.append(w[pos:])
    # a one- or two-digit number glued to the end of a Japanese word
    split = []
    for w in out:
        m = re.match(r"^([" + JP + r"]+)(\d{1,2})$", w)
        if m:
            split += [m.group(1), m.group(2)]
        else:
            split.append(w)
    return split


def _is_marker(w):
    return w in ("season", "part", "cour", "stage")


def _ordinal_of(w):
    """A word read as an ordinal where one may stand alone: a suffixed
    number, a spelled ordinal, a Roman numeral, a bare number of one or
    two digits (a zero included), a Japanese division."""
    m = re.fullmatch(r"(\d+)(?:st|nd|rd|th)", w)
    if m:
        return [int(m.group(1))]
    if w in WORD_ORD:
        return [WORD_ORD[w]]
    if w in ROMAN:
        return [ROMAN[w]]
    if re.fullmatch(r"\d{1,2}", w):
        return [int(w)]
    m = _JP_DIV.fullmatch(w)
    if m:
        k = _jp_ordinal(m.group(1))
        return None if k is None else [k]
    return None


def _divisions(tokens):
    """Each token read as a word (`None`) or as the ordinals of the
    division it opens, with the tokens the division spans."""
    items = []
    i = 0
    while i < len(tokens):
        w = tokens[i]
        nxt = tokens[i + 1] if i + 1 < len(tokens) else None
        if _is_marker(w) and nxt is not None:
            if _SPAN.fullmatch(nxt):
                items.append(([int(k) for k in nxt.split("+")], 2))
                i += 2
                continue
            if nxt.isdigit():
                items.append(([int(nxt)], 2))
                i += 2
                continue
            if nxt in CARD:
                items.append(([CARD[nxt]], 2))
                i += 2
                continue
        if (re.fullmatch(r"\d+(?:st|nd|rd|th)", w) or w in WORD_ORD) and nxt is not None and (
            _is_marker(nxt) or nxt == "half"
        ):
            items.append((_ordinal_of(w), 2))
            i += 2
            continue
        if w in WORD_ORD:
            # a word ordinal standing alone ends the name only where
            # nothing but divisions follows it
            rest = tokens[i + 1 :]
            if all(_ordinal_of(r) is not None or _is_marker(r) or r == "half" for r in rest):
                items.append(([WORD_ORD[w]], 1))
            else:
                items.append((None, 1))
            i += 1
            continue
        if w == "x":
            # X is never a numeral, being a word of its own in titles like "Spy x Family"
            items.append((None, 1))
            i += 1
            continue
        k = _ordinal_of(w)
        if k is not None and not re.fullmatch(r"\d{3,}", w):
            items.append((k, 1))
        else:
            items.append((None, 1))
        i += 1
    return items


class Reading:
    """What one reader makes of a title: its name (words up to the first
    division), the ordinals it reads as (a division whose first ordinal is
    1 left out), the words past the name outside its divisions, the
    primes it is written with and the release year its tags name."""

    def __init__(self, title):
        n = norm(title)
        tokens = _tokenize(n)
        items = _divisions(tokens)
        self.name, self.ords, self.rest = [], [], []
        i = 0
        in_name = True
        for ords, width in items:
            if ords is None:
                (self.name if in_name else self.rest).append(tokens[i])
            else:
                if in_name and not self.name:
                    # a title that opens on a division is named by its stem
                    stem = _stem_tokens(tokens)
                    self.name = list(stem)
                    in_name = False
                    # read on from the stem's end
                    tail_items = _divisions(tokens[len(stem) :])
                    self.ords = [o for o in _ordinals_of(_divisions(stem)) if o]
                    self.ords += [o for o in _ordinals_of(tail_items) if o]
                    j = len(stem)
                    for t_ords, t_width in tail_items:
                        if t_ords is None:
                            self.rest.append(tokens[j])
                        j += t_width
                    break
                in_name = False
                if ords[0] != 1:
                    self.ords.append(ords[0] if len(ords) == 1 else tuple(ords))
            i += width
        low = title.lower()
        tags = [_ascii(m.group(0))[1:-1].strip() for m in CTAG.finditer(low)]
        years = [t for t in tags if t.isdigit()]
        self.year = years[0] if years else None
        ends = len(re.findall(r"(?<=[0-9a-z" + JP + r"])['’′](?![0-9a-z" + JP + r"])", low))
        opens = len(re.findall(r"(?<![0-9a-z" + JP + r"])['’′](?=[0-9a-z" + JP + r"])", low))
        self.primes = max(0, ends - opens)

    def same_show(self, other):
        """`other` is named by this title: same name word by word, read
        alike, the same words past the name, the same primes, and no
        conflicting release year."""
        return (
            self.name == other.name
            and self.ords == other.ords
            and self.rest == other.rest
            and self.primes == other.primes
            and not (self.year and other.year and self.year != other.year)
        )


def _ordinals_of(items):
    out = []
    for ords, _ in items:
        if ords is None:
            continue
        out.append(None if ords[0] == 1 else (ords[0] if len(ords) == 1 else tuple(ords)))
    return out


def _stem_tokens(tokens):
    """The title with every marker and Japanese division it ends on
    removed; a bare or glued number it ends on stays."""
    items = _divisions(tokens)
    end = len(items)
    while end > 0:
        ords, width = items[end - 1]
        token = tokens[sum(w for _, w in items[: end - 1])]
        if ords is not None and (width == 2 or _JP_DIV.fullmatch(token) or token in ROMAN):
            end -= 1
            continue
        break
    return tokens[: sum(w for _, w in items[:end])]


def names(titles, cand):
    """The entry names a title when one of its titles carries the title's
    name, reads as it does, carries the same words past that name, the
    same primes and no conflicting release year — under any alias."""
    rc = Reading(cand)
    return any(Reading(t).same_show(rc) for t in titles)


# --- choosing among candidates

PARTS = (
    re.compile(r"\s*\bpart\s+(\d+)$"),
    re.compile(r"\s*\b(\d+)(?:st|nd|rd|th)\s+part$"),
    re.compile(r"\s*第([0-9]+|[〇零一二三四五六七八九十百]+)部$"),
)


def parts_of(t):
    """Every part marker in the run of markers a title ends on; none reads
    as the first part."""
    n = _word_ordinals(t)
    out = set()
    while True:
        for r in PARTS:
            m = r.search(n)
            if m and n[: m.start()].strip():
                k = _jp_ordinal(m.group(1))
                if k is not None:
                    out.add(k)
                n = n[: m.start()].rstrip(" :-")
                break
        else:
            for r in TRAIL:
                m = r.search(n)
                if m and n[: m.start()].strip():
                    n = n[: m.start()].rstrip(" :-")
                    break
            else:
                return out


def part_agrees(titles, cand):
    eparts = set().union(*(parts_of(t) for t in titles)) or {1}
    return bool((parts_of(cand) or {1}) & eparts)


def head(titles, rows, counted):
    """The probe head: at most five hits that could be picked and, with a
    count, at most five more as evidence — the first five that could not
    be picked, up to the fifth that could."""
    picked, evidence = [], []
    for r in rows:
        if pickable(titles, r[1]):
            picked.append(r)
            if len(picked) == 5:
                break
        elif counted and len(evidence) < 5:
            evidence.append(r)
    return picked, evidence


def countless_pick(titles, searched, rows, entry_year):
    """The countless pick for the pool `searched` answered: the exact
    title; failing one, among the hits something vouches for — its own
    year the entry's, or the entry naming it — part agreement, then the
    provider's order (every listing's year is the same here)."""
    adm, _ = head(titles, rows, False)
    adm = [r for r in adm if pickable_as_read(titles, r[1])]
    if not adm:
        return None
    exact = [r for r in adm if written(r[1]) == written(searched)]
    if exact:
        return exact[0]
    vouched = [r for r in adm if entry_year == LISTING_YEAR or names(titles, r[1])]
    if not vouched:
        return None
    return sorted(vouched, key=lambda r: 0 if part_agrees(titles, r[1]) else 1)[0]


def vouched_by_name(titles, searched, cand):
    """The title searched, or a title the entry names — what alone vouches
    for a hit when the entry's count stands alone. The searched half is
    the docs' wording; in every generated pool it is subsumed by the
    other, since one of the entry's titles always names itself, so the
    agreement says nothing about how the two sides compare the title
    searched."""
    return written(cand) == written(searched) or names(titles, cand)


def card_names(case, slug):
    """The names the driver gives a hit beside its listing's title: the
    romanized name its search card carries — the entry's canonical title
    on every card, as the provider's cards for an entry's own listings
    carry the entry's romaji, unless the pool says otherwise (`jnames`,
    slug to name; a slug it leaves out carries none) — and the names its
    entry page gives (`page_names`, slug to names)."""
    j = case.get("jnames")
    card = [case["titles"][0]] if j is None else ([j[slug]] if slug in j else [])
    return card + list(case.get("page_names", {}).get(slug, []))


def _starts_with_name(title, other):
    """`title` starts with `other`'s name — at a word's end, or with a
    digit or 第 glued to it, no digit when the name itself ends in one
    ("Show2" and "ショー第2期" start with "Show" and "ショー", "Showtime"
    does not, nor "861" with "86"); a name of no words — a title that is
    nothing but a division — starts nothing."""
    nm = Reading(other).name
    if not nm:
        return False
    toks = _tokenize(norm(title))
    if len(toks) < len(nm) or toks[: len(nm) - 1] != nm[:-1]:
        return False
    last, word = nm[-1], toks[len(nm) - 1]
    if word == last:
        return True
    rest = word[len(last):] if word.startswith(last) else None
    return bool(rest) and (rest[0] == "第" or (rest[0].isdigit() and not last[-1].isdigit()))


def shares_name(titles, cand):
    """One of the entry's titles and `cand` share a name: one starts with
    the other's name, whatever follows it."""
    return any(_starts_with_name(cand, t) or _starts_with_name(t, cand) for t in titles)


def needs_a_name(count, entry_year):
    """"A year and a count are no identity either": a count above one,
    and a year."""
    return count is not None and count > 1 and entry_year is not None


def vouched_dated(titles, searched, cand, others):
    """The title searched, or a name of the show — the listing's title,
    its card's romanized name, its page's names (`others`) — that the
    entry names or shares a name with: what vouches for a hit when the
    entry has a year and a count."""
    return written(cand) == written(searched) or any(
        names(titles, n) or shares_name(titles, n) for n in [cand] + list(others)
    )


def count_stands_alone(count, entry_year):
    """"A count with no year is no identity either": a count above one,
    and no year."""
    return count is not None and count > 1 and entry_year is None


def counted_pick(titles, searched, rows, count, entry_year, names_of=lambda slug: []):
    """The pick with a count and no broad listing: the best distance within
    the tolerance among the hits that could be picked; ties prefer the
    exact title, then part agreement, then the provider's order. When the
    count stands alone, only a hit the title searched or the entry's names
    vouch for can be picked; with a year beside the count, only a hit the
    title searched or a name of the show (`names_of`) vouches for."""
    adm, _ = head(titles, rows, True)
    adm = [r for r in adm if pickable_as_read(titles, r[1])]
    if count_stands_alone(count, entry_year):
        adm = [r for r in adm if vouched_by_name(titles, searched, r[1])]
    if needs_a_name(count, entry_year):
        adm = [r for r in adm if vouched_dated(titles, searched, r[1], names_of(r[0]))]
    tol = max(3, count // 10)
    fits = [r for r in adm if abs(r[2] - count) <= tol]
    if not fits:
        return None, []
    best = min(abs(r[2] - count) for r in fits)
    tied = [r for r in fits if abs(r[2] - count) == best]
    return sorted(
        tied,
        key=lambda r: (
            0 if written(r[1]) == written(searched) else 1,
            0 if part_agrees(titles, r[1]) else 1,
        ),
    )[0], tied


def with_versions(r, tied):
    """The expectation for the pick `r`: copies of it under another version
    tag among `tied` make it a pick with versions, the uncensored copy
    playing (Versions of one pick)."""
    versions = versions_of(r, tied)
    if not versions:
        return {"pick": r[0], "count": r[2]}
    playing = next(v[0] for v in versions if v[1] == "uncensored")
    return {"pick": playing, "count": r[2], "versions": versions}


def _stem_text(t):
    return strip_trailing(_word_ordinals(t))[0]


def _opens_past_stem(title, stem):
    """`title` starts with `stem` at a word's end, or with a digit or 第
    glued to it (no digit when the stem itself ends in one)."""
    n = norm(title)
    if not n.startswith(stem) or len(n) == len(stem):
        return n == stem
    nxt = n[len(stem)]
    if nxt == "第" or not nxt.isalnum():
        return True
    return nxt.isdigit() and not stem[-1].isdigit()


def predict(case):
    """The pick the docs answer for a pool, or `NOT_MODELLED` for a shape
    the predicates do not read."""
    titles, count, year = case["titles"], case["count"], case["entry_year"]
    rows = [tuple(r) for r in case["rows"]]
    if case.get("dead") or count == 1:
        return NOT_MODELLED
    if count is not None:
        tol = max(3, count // 10)
        for w in rows:
            stem = _stem_text(w[1])
            # a title that does not start with W's whole stem is read past
            # it without the one- or two-digit bare number the stem ends on
            short = re.sub(r"\s*(?<!\d)\d{1,2}$", "", stem)
            stems = {stem, short} if short.strip() else {stem}
            kin = [r for r in rows if r is not w and any(_opens_past_stem(r[1], s) for s in stems)]
            # a broad listing a sibling could complete: the spanning cut
            if w[2] > count and admitted(titles, w[1]) and kin:
                return NOT_MODELLED
            # an admitted bare title beside later parts of it — the title
            # followed by one short part marker — whose counts could
            # together explain the entry's, within the tolerance or short
            # of it: stitching. A refused lead heads no chain, and a first
            # part completes none.
            parts = [r for r in rows if r is not w and part_after(r[1], _word_ordinals(w[1])) not in (None, 1)]
            if admitted(titles, w[1]) and parts and w[2] + min(r[2] for r in parts) <= count + tol:
                return NOT_MODELLED
    for searched in titles:
        if count is None:
            r = countless_pick(titles, searched, rows, year)
            if r:
                return {"pick": r[0], "count": r[2]}
        else:
            r, tied = counted_pick(titles, searched, rows, count, year, lambda slug: card_names(case, slug))
            if r:
                return with_versions(r, tied)
    return None


def clean_miss_expected(case):
    """Whether a pool the walk refused is the clean miss the availability
    cache persists: not when the title rule refused a hit — one the count
    accepted, among the hits probed; without a count, any hit at all."""
    titles, count = case["titles"], case["count"]
    rows = [tuple(r) for r in case["rows"]]
    if count is None:
        # a hit the head took that the title rule refuses past its
        # version tag is passed over, a refusal by title as a dropped one is
        taken, _ = head(titles, rows, False)
        late = any(not admitted_as_read(titles, r[1]) for r in taken)
        return not late and not any(not admitted(titles, r[1]) for r in rows)
    taken, evidence = head(titles, rows, True)
    tol = max(3, count // 10)
    if any(abs(r[2] - count) <= tol and not admitted(titles, r[1]) for r in evidence):
        return False
    if any(abs(r[2] - count) <= tol and not admitted_as_read(titles, r[1]) for r in taken):
        return False
    # the count-alone refusal is a refusal by title too: a fitting hit
    # nothing names, among the hits probed, leaves no clean miss
    if count_stands_alone(count, case["entry_year"]):
        adm, _ = head(titles, rows, True)
        return not any(abs(r[2] - count) <= tol for r in adm)
    # so is the dated-name refusal: a fitting hit no name vouches for —
    # under any title searched, since every title's search answers with
    # the same rows here and the walk reads every pool
    # — any hit the year and count would have taken: within the
    # tolerance, or from the entry's year (every listing's here) and
    # shorter than the entry, as the airing rescue and the stitching take
    if needs_a_name(count, case["entry_year"]):
        adm, _ = head(titles, rows, True)
        confirmed = case["entry_year"] == LISTING_YEAR
        return not any(
            (abs(r[2] - count) <= tol or (confirmed and r[2] < count))
            and not all(vouched_dated(titles, s, r[1], card_names(case, r[0])) for s in titles)
            for r in adm
        )
    return True


def refusal_expected(case):
    """The error and verdict a refused pool answers with. A pool with a
    listing whose probe dies is unheard — the transient network failure it
    is, never the clean miss the availability cache persists. Any other
    refusal is NoResults, clean unless the title rule refused a fitting
    hit."""
    if case.get("dead"):
        return "Network", False
    return "NoResults", clean_miss_expected(case)


def episodes_expected(count):
    """The rows a pick of `count` regular episodes lists: every generated
    listing numbers its rows 1..n as `[id, number, tag]` with id = number
    and no tag, and a pick plays the first `count` of them — a whole
    listing, or a broad one cut to the entry's own episodes from its start
    with its own numbers kept. A stitched pick lists otherwise — each row
    keeps the id it streams from in its own part — so `expect` cannot
    carry one: stitching is tabled out (`NOT_MODELLED`), never expected."""
    return [[k, k, None] for k in range(1, count + 1)]


# --- the generated set

NUMERIC_STEMS = ["86", "Mob Psycho 100", "Kaiju No. 8", "Show 2", "Lucky 2", "ショー２"]
PLAIN = ["Show", "Show Other", "Other Show"]
SEQUEL_ENTRIES = [
    "Show 2nd Season",
    "Mob Psycho 100 II",
    "86 Part 2",
    "Kaiju No. 8 Season 2",
    "Lucky 2 Season 2",
    "Show 2 Season 2",
    "ショー第2期",
]


def title_sets(t):
    yield "one", [t]
    yield "alias-only", ["Zzyzx Alias", t]
    yield "cross-script", ["ショウ ゼータ", t]


def suffixes(t):
    out = [
        "",
        " Recap",
        " Part 1",
        " Season 1",
        " Part 2",
        " Season 2",
        " 2nd Season",
        " Season 3",
        " 3rd Season",
        "第2期",
        " 第2期",
        " 0",
        " 5",
        " 20",
        " 2.5",
        ": 2nd Season",
        " (2020)",
    ]
    if re.search(r"[0-9０-９]$", t):
        out += ["0", "1", "5", "00", "０", "１"]
    return out


def first_num_variants(t):
    """The entry's first number continued, bumped or lowered, spaced or
    glued as written."""
    n = norm(t)
    m = re.search(r"\d+", n)
    if not m:
        return []
    num = m.group(0)
    s, e = m.span()
    return [
        t[:s] + rep + t[e:]
        for rep in [num + "0", num + "1", num + "00", str(int(num) + 1), str(max(int(num) - 1, 0))]
    ]


UNREL = ("zeta", "Zeta Thing", 24)


def case_id(family, titles, count, year, rows, dead, names=None):
    """A pool's id follows its content, not its position in the set, so
    the allowlist survives the generator growing. The names a pool gives
    its hits (`jnames`, `page_names`) are part of it when it gives any."""
    content = [titles, count, year, [list(r) for r in rows], list(dead)]
    if names is not None:
        content.append(names)
    body = json.dumps(content, ensure_ascii=False, sort_keys=True)
    return family.split("/")[0] + "/" + hashlib.sha1(body.encode("utf-8")).hexdigest()[:10]


def hand_written():
    """The case sets written by hand under cases/, each pool carrying the
    pick the docs' tables answer for it."""
    out = []
    cases_dir = os.path.join(HERE, "cases")
    for name in sorted(os.listdir(cases_dir)):
        if not name.endswith(".json"):
            continue
        with open(os.path.join(cases_dir, name), encoding="utf-8") as fh:
            for c in json.load(fh):
                out.append(dict(c))
    return out


def gen():
    cases = []

    def add(titles, count, year, rows, expect, fam, dead=(), names=None):
        rows = [list(r) for r in rows]
        case = dict(
            id=case_id(fam, titles, count, year, rows, dead, names),
            titles=list(titles),
            count=count,
            entry_year=year,
            rows=rows,
            expect=expect,
            family=fam,
        )
        if dead:
            case["dead"] = list(dead)
        if names is not None:
            case.update(names)
        cases.append(case)

    entries = NUMERIC_STEMS + PLAIN + SEQUEL_ENTRIES
    for t in entries:
        cands = sorted(set([t + s for s in suffixes(t)] + first_num_variants(t)))
        for tsname, titles in title_sets(t):
            for c in cands:
                x = ("x", c, E)
                # with a count: the candidate is the only exact fit; nothing heads a W
                for pos, rows in (("alone", [x]), ("unrel-first", [UNREL, x]), ("unrel-last", [x, UNREL])):
                    case = dict(titles=titles, count=E, entry_year=LISTING_YEAR, rows=rows)
                    add(titles, E, LISTING_YEAR, rows, predict(case), f"admission/{tsname}/{pos}")
                # with a count and no year: the count stands alone, so only
                # the title searched or a name of the entry's vouches; an
                # unrelated hit that fits the count sits beside the
                # candidate so the refusal of what nothing names is seen
                fit = ("u", "Zeta Thing", E)
                for pos, rows in (("alone", [x]), ("fit-first", [fit, x]), ("fit-last", [x, fit])):
                    case = dict(titles=titles, count=E, entry_year=None, rows=rows)
                    add(titles, E, None, rows, predict(case), f"counted-noyear/{tsname}/{pos}")
                # with a count and a year, the cards naming nothing: only
                # the title searched or a name of the show vouches, and an
                # unrelated hit that fits the count sits beside the
                # candidate — named by nothing, by its card, or by its page
                for how, names in (
                    ("unnamed", {"jnames": {}}),
                    ("card", {"jnames": {"u": titles[0]}}),
                    ("page", {"jnames": {}, "page_names": {"u": [titles[0]]}}),
                ):
                    for pos, rows in (("alone", [x]), ("fit-first", [fit, x]), ("fit-last", [x, fit])):
                        case = dict(titles=titles, count=E, entry_year=LISTING_YEAR, rows=rows, **names)
                        add(titles, E, LISTING_YEAR, rows, predict(case), f"counted-dated/{tsname}/{how}/{pos}", names=names)
                # countless, with and without a year
                for year in (LISTING_YEAR, None):
                    for pos, rows in (("alone", [x]), ("unrel-first", [UNREL, x]), ("unrel-last", [x, UNREL])):
                        case = dict(titles=titles, count=None, entry_year=year, rows=rows)
                        add(
                            titles,
                            None,
                            year,
                            rows,
                            predict(case),
                            f'countless/{tsname}/{"year" if year else "noyear"}/{pos}',
                        )
    # the spanning cut over numeric stems the entry heads (W = the entry's title, 24)
    for t in NUMERIC_STEMS + ["Show"]:
        ends_digit = bool(re.search(r"[0-9０-９]$", t))
        shapes = []
        shapes.append((t + " Part 2", "cut"))  # L: a later part past W's stem
        shapes.append((t + " Season 3", "cut") if t != "Show" else (t + " Season 2", "cut"))
        if ends_digit:
            jp = t.endswith("２")
            # "Lucky 2 2nd Season" reads [2, 2]: L. "ショー２第2期": the ２ is inside a word,
            # as 怪獣８号's ８ is, so it reads [2] as the entry does and carries W's stem
            # opening a division: O.
            shapes.append((t + ("第2期" if jp else " 2nd Season"), "O" if jp else "cut"))
            for g in ["０", "１"] if jp else ["0", "1"]:
                shapes.append((t + g, "nothing"))  # no digit glued to a digit-ending stem; refused
            if jp:
                shapes.append((t + "０第2部", "nothing"))  # ショー２０第2部
            else:
                shapes.append((t + "0 Part 2", "nothing"))  # Show 20 Part 2: only continues W's number
            shapes.append((t + " Season 1", "O"))  # reads as the entry, W's stem + a first division
        else:
            shapes.append((t + " Part 1", "O"))
        if t in ("Show 2", "Lucky 2", "Kaiju No. 8"):
            m = re.search(r"\d+$", t)
            shapes.append((t[: m.start()] + str(int(m.group(0)) + 1), "cut"))  # Lucky 3 beside Lucky 2
        for c, kind in shapes:
            for pos in ("none", "first", "mid", "last"):
                w = ("w", t, 2 * E)
                x = ("x", c, E)
                rows = {"none": [w, x], "first": [UNREL, w, x], "mid": [w, UNREL, x], "last": [w, x, UNREL]}[pos]
                exp = {"cut": {"pick": "w", "count": E}, "O": {"pick": "x", "count": E}, "nothing": None}[kind]
                add([t], E, LISTING_YEAR, rows, exp, f"cut/{kind}/{pos}")
    # the doc's own rows for the continued-number rule
    for t, rows in [
        ("Show 2", [("w", "Show 2", 24), ("x", "Show20", 12)]),
        ("Mob Psycho 100 II", [("w", "Mob Psycho 100 II", 22), ("x", "Mob Psycho 1000 Part 2", 10)]),
        ("86 Part 2", [("w", "86 Part 1", 24), ("x", "861", 12)]),
        ("Lucky 2", [("w", "Lucky 2", 24), ("x", "Lucky 2 2nd Season", 12)]),
        ("Kaiju No. 8", [("w", "Kaiju No. 8", 23), ("p", "Kaiju No. 8 Part 2", 11), ("x", "Kaiju No. 8 Season 1", 12)]),
    ]:
        exp = {"Lucky 2": {"pick": "w", "count": 12}, "Kaiju No. 8": {"pick": "x", "count": 12}}.get(t)
        add([t], E, LISTING_YEAR, rows, exp, "docrow")
    # "or the number another of the entry's titles carries after the same words"
    for titles, c, exp in [
        (["Show 2", "Other Name 20"], "Show20", None),
        (["Show 2", "Show 20"], "Show20", {"pick": "x", "count": E}),
    ]:
        add(titles, E, LISTING_YEAR, [("x", c, E)], exp, "carried-by-another-title")
    # an unnumbered sequel name adds a word or a mark to the name: not the entry before it
    for t, c in [
        ("Gintama 2", "Gintama'"),
        ("Gintama Season 2", "Gintama'"),
        ("Kaguya-sama wa Kokurasetai 2", "Kaguya-sama wa Kokurasetai?"),
        ("Haikyuu!! 4th Season", "Haikyuu!! To the Top"),
        ("Steins;Gate 0", "Steins;Gate Zero"),
        ("Show 2", "Show Two"),
        ("Show V Something", "Show Five Something"),
        ("Kaiju No. 8", "Kaiju No. Hachi"),
        ("Final Fantasy VII: Advent Children", "Final Fantasy: Advent Children"),
    ]:
        add([t], E, LISTING_YEAR, [("x", c, E)], {"pick": "x", "count": E}, "unnumbered-sequel-name")
    for t, c in [
        ("Haikyuu!! 2nd Season", "Haikyuu!!"),
        ("Show 2", "Show Season 1"),
        ("Show 2", "Show 1st Season"),
        ("Show 2", "Show"),
        ("Steins;Gate 0", "Steins;Gate"),
        ("ショー第2期", "ショー第1期"),
        ("Show Part 2", "Show Season 1 Part 1"),
        ("Show 2", "Show First Half"),
        ("Show 2", "Show Season 1 First Half"),
        ("Show 2", "Show: Season 1"),
        ("Oshi no Ko Season 2", "Oshi no Ko: Season 1"),
        ("Show: Season 2", "Show Season 1"),
        ("Steins;Gate 0", "Steins Gate"),
        ("Show 2", "Show - Season 1"),
    ]:
        add([t], E, LISTING_YEAR, [("x", c, E)], None, "name-and-first-divisions")
    # a bracketed year or format tag is no word to the entry before; other bracketed words are
    for c, exp in [
        ("Show （ＴＶ）", None),
        ("Show Season 1 （２０２０）", None),
        ("Show Season 1 (2020)", None),
        ("Show (TV)", None),
        ("Show [OVA]", None),
        ("Show (2020) Season 1", None),
        ("Show （ONA）", None),
        ("Show (Shin Series)", {"pick": "x", "count": E}),
        ("Show (Movie)", {"pick": "x", "count": E}),
    ]:
        add(["Show 2"], E, LISTING_YEAR, [("x", c, E)], exp, "year-or-format-tag")
    # a candidate naming a season before the entry's own is never picked: a clean miss
    for entry, rows, exp in [
        ("Show Season 3 Part 2", [("s", "Show Season 2 Part 2", E)], None),
        ("Show Season 3 Part 2", [("s", "Show Season 2", E)], None),
        ("Show Season 3 Part 2", [("s", "Show Season 1 Part 2", E)], None),
        ("Show 3 Part 2", [("s", "Show 2 Part 2", E)], None),
        ("Show Season 3 Part 2", [("w", "Show Season 3", 24), ("s", "Show Season 2 Part 2", E)], None),
        ("ショー 第3期 第2部", [("s", "ショー第二期第2部", E)], None),
        ("Show 3rd Season Part 2", [("s", "Show 2nd Season Part 2", E)], None),
        ("Show Season 3 Part 2", [("s", "Show Season 3 Part 2", E)], {"pick": "s", "count": E}),
        ("Show Season 3 Part 2", [("s", "Show 3rd Season Part 2", E)], {"pick": "s", "count": E}),
        ("86 Part 2", [("x", "Eighty Six: 2nd Season", E)], {"pick": "x", "count": E}),
        ("Kaiju No. 8 Season 2", [("x", "Kaiju No. 8 2nd Season", E)], {"pick": "x", "count": E}),
    ]:
        for cnt in (E, None):
            add([entry], cnt, LISTING_YEAR, rows, exp, "season-before-entry")
    # a dead candidate that could never be picked blocks no winner
    for entry, dead, own in [
        ("Show 2", "Show", "Show 2nd Season"),
        ("Show 2", "Show Season 1", "Show 2nd Season"),
        ("Show 2", "Show First Half", "Show 2nd Season"),
        ("Show Season 3 Part 2", "Show Season 2 Part 2", "Show 3rd Season Part 2"),
        ("Show Season 3 Part 2", "Show Season 2", "Show 3rd Season Part 2"),
        ("Show Season 2", "Show Season 3", "Show 2nd Season"),
    ]:
        add(
            [entry],
            E,
            LISTING_YEAR,
            [("dead", dead, E), ("own", own, E)],
            {"pick": "own", "count": E},
            "dead-candidate",
            dead=["dead"],
        )
    # a dead candidate that could be picked, tying the winner's rank from an
    # earlier position, makes the pick transient
    add(
        ["Show 2"],
        E,
        LISTING_YEAR,
        [("dead", "Show 2nd Season", E), ("own", "Show Season 2", E)],
        None,
        "dead-candidate",
        dead=["dead"],
    )
    # a broad listing that is another entry of the same show is never cut
    for titles, rows, exp in [
        (
            ["Show Season 3", "Show 2nd Season"],
            [("w", "Show Second Season", 24), ("l", "Show Second Season Part 2", E)],
            None,
        ),
        (
            ["Show Season 3", "Show 2nd Season"],
            [("w", "Show 2nd Season", 24), ("l", "Show 2nd Season Part 2", E)],
            {"pick": "w", "count": E},
        ),
        (
            ["Show Season 3", "Show Season 3"],
            [("w", "Show Season 3", 24), ("l", "Show Season 3 Part 2", E)],
            {"pick": "w", "count": E},
        ),
    ]:
        add(titles, E, LISTING_YEAR, rows, exp, "cut-another-entry")
    # a catalogue tag tells a remake from its original where titles are compared as written
    for entry, cnt, rows, want in [
        ("Hunter x Hunter", None, [("new", "Hunter x Hunter (2011)", 148), ("old", "Hunter x Hunter", 62)], "old"),
        ("Hunter x Hunter (2011)", None, [("old", "Hunter x Hunter", 62), ("new", "Hunter x Hunter (2011)", 148)], "new"),
        ("Gintama", None, [("new", "Gintama (2011)", 51), ("old", "Gintama", 201)], "old"),
        ("Gintama (2011)", None, [("old", "Gintama", 201), ("new", "Gintama (2011)", 51)], "new"),
        ("Fruits Basket", 25, [("new", "Fruits Basket (2019)", 25), ("old", "Fruits Basket", 25)], "old"),
        ("Fruits Basket (2019)", 25, [("old", "Fruits Basket", 25), ("new", "Fruits Basket (2019)", 25)], "new"),
        ("Show (2011)", E, [("other", "Show", E), ("own", "Show (2011)", E)], "own"),
        ("Show (2011) Season 2", E, [("orig", "Show Season 2", E), ("own", "Show Season 2 (2011)", E)], "own"),
        ("Show (2011) Season 2", None, [("orig", "Show Season 2", E), ("own", "Show Season 2 （２０１１）", E)], "own"),
    ]:
        for yr in (LISTING_YEAR, None):
            add(
                [entry],
                cnt,
                yr,
                rows,
                {"pick": want, "count": [r[2] for r in rows if r[0] == want][0]},
                "remake",
            )
    # the probe head: another entry of the same show is never charged to the five
    befores = [
        ("a", "Show", E),
        ("b", "Show Season 1", E),
        ("c", "Show Part 1", E),
        ("d", "Show First Season", E),
        ("e", "Show First Half", E),
    ]
    for cnt in (E, None):
        add(["Show 2"], cnt, LISTING_YEAR, befores + [("own", "Show 2", E)], {"pick": "own", "count": E}, "probe-head")
        add(
            ["Show 2"],
            cnt,
            LISTING_YEAR,
            befores + [("f", "Show 1st Season", E), ("own", "Show 2", E)],
            {"pick": "own", "count": E},
            "probe-head",
        )
        seasons = [
            ("a", "Show Season 2", E),
            ("b", "Show Season 2 Part 2", E),
            ("c", "Show 2nd Season", E),
            ("d", "Show Season 1 Part 2", E),
            ("e", "Show Second Season", E),
        ]
        add(
            ["Show Season 3 Part 2"],
            cnt,
            LISTING_YEAR,
            seasons + [("own", "Show Season 3 Part 2", E)],
            {"pick": "own", "count": E},
            "probe-head",
        )
    # five pickable admitted hits still fill the five: the sixth is not considered
    add(
        ["Show 2"],
        E,
        LISTING_YEAR,
        [(f"r{i}", f"Show 2 Arc {c}", 50) for i, c in enumerate("ABCDE")] + [("own", "Show 2", E)],
        None,
        "probe-head",
    )
    # a sibling naming a season before W's is a P whatever its part: nothing is cut
    for entry, wide, sib in [
        ("Show Season 3", "Show Season 3", "Show Season 2 Part 2"),
        ("Show Season 3", "Show Season 3", "Show II Part 2"),
        ("Show Season 3", "Show Season 3", "Show Second Season Part 2"),
        ("Show Season 3", "Show Season 3", "Show Part 2 Season 2"),
        ("Show Season 3", "Show Season 3", "Show 2nd Season 2nd Part"),
        ("Show Season 3", "Show Season 3", "Show Season Two Part Two"),
        ("ショー 第3期", "ショー第3期", "ショー第二期第2部"),
        ("ショー 第3期", "ショー第3期", "ショー第2期第2部"),
        ("ショー 第3期", "ショー第3期", "ショー 第2期 第2部"),
        ("Show 3", "Show 3", "Show 2 Part 2"),
    ]:
        add([entry], E, LISTING_YEAR, [("w", wide, 24), ("s", sib, E)], None, "earlier-season-with-part")
        add([entry], E, LISTING_YEAR, [("s", sib, E), ("w", wide, 24)], None, "earlier-season-with-part")
    # a title inference that scores out a fit is never a clean miss
    for t, rows in [
        ("Show Season 3 Part 1", [("w", "Show Season 3 Part 1", 24), ("p", "Show Season 3 Part 1 2nd Half", 12)]),
        ("Show 2", [("x", "Show Season 1", 12)]),
        ("Show 2", [("x", "Show Season 1", 40)]),
    ]:
        add([t], E, LISTING_YEAR, rows, None, "other-entry-clean")
    # a copy of the pick under a version tag beside it — after it or before
    # it at the same count, after it one short — the tag in each spelling in
    # turn, in every pool whose one hit is the pick (Versions of one pick); a
    # card that names the pick names its copy too
    for i, c in enumerate(list(cases)):
        fam = c["family"].split("/")[0]
        if fam not in ("admission", "counted-dated", "counted-noyear") or len(c["rows"]) != 1:
            continue
        x = c["rows"][0]
        vtag = ("(Uncensored)", "[UNCENSORED]", "（Uncensored）")[i % 3]
        same = [x[0] + "-u", f"{x[1]} {vtag}", x[2]]
        short = [x[0] + "-u", f"{x[1]} {vtag}", x[2] - 1]
        names = {k: c[k] for k in ("jnames", "page_names") if k in c} or None
        if names and x[0] in names.get("jnames", {}):
            names = dict(names, jnames=dict(names["jnames"], **{same[0]: names["jnames"][x[0]]}))
        for how, rows in (("same/after", [x, same]), ("same/before", [same, x]), ("short/after", [x, short])):
            case = dict(c, rows=rows, **(names or {}))
            want = predict(case)
            if want is NOT_MODELLED:
                continue
            add(c["titles"], c["count"], c["entry_year"], rows, want, f"versions/{fam}/{how}", names=names)
    # every reader reads a title without its catalogue tags — every family again, tagged
    tags = ["(TV)", "(2024)", "[OVA]", "（ＯＮＡ）", "【OAD】", "（２０２０）"]
    tagged = []
    for i, c in enumerate(list(cases)):
        if c["family"].startswith(("year-or-format-tag", "remake", "versions")):
            continue
        # a title already carrying a tag is not tagged again: two release
        # years on one title is a shape the docs do not speak to
        if any(CTAG.search(x.lower()) for x in c["titles"] + [r[1] for r in c["rows"]]):
            continue
        tag = tags[i % len(tags)]
        fam = "tagged-" + c["family"].split("/")[0]
        titles = [f"{x} {tag}" for x in c["titles"]]
        rows = [[r[0], f"{r[1]} {tag}", r[2]] for r in c["rows"]]
        t = dict(c, family=fam, titles=titles, rows=rows)
        names = {k: c[k] for k in ("jnames", "page_names") if k in c} or None
        t["id"] = case_id(fam, titles, c["count"], c["entry_year"], rows, c.get("dead", []), names)
        tagged.append(t)
    out = {}
    for c in cases + tagged + hand_written():
        if c["id"] in out:
            if out[c["id"]] != c:
                raise SystemExit(f"oracle: two pools share the id {c['id']} and differ")
            continue
        out[c["id"]] = c
    return list(out.values())


# --- the comparison

def outcome(got):
    """A driver outcome as the pick it reports, with its versions when it
    reports any, or `None` for a refusal."""
    if "ok" in got:
        out = {"pick": got["ok"], "count": got["count"]}
        if "versions" in got:
            out["versions"] = got["versions"]
        return out
    return None


def summary(got):
    """What the allowlist records of an outcome: the pick with its
    episodes and any versions, or the refusal's error and clean-miss
    verdict."""
    if "ok" in got:
        out = {"pick": got["ok"], "count": got["count"], "episodes": got["episodes"]}
        if "versions" in got:
            out["versions"] = got["versions"]
        return out
    return {"refused": got["err"], "clean_miss": got["clean_miss"]}


def compare(cases_path, out_path, allowlist_path=None):
    """Reads the driver's outcomes against the pools' expectations and the
    allowlist; prints the report; returns the exit code."""
    with open(cases_path, encoding="utf-8") as fh:
        cases = {c["id"]: c for c in json.load(fh)}
    with open(out_path, encoding="utf-8") as fh:
        got = {g["id"]: g["got"] for g in json.load(fh)}
    allow = {}
    problems = []
    if allowlist_path:
        with open(allowlist_path, encoding="utf-8") as fh:
            for entry in json.load(fh):
                if not entry.get("reason", "").strip():
                    problems.append(f"allowlist entry {entry.get('id')!r} gives no reason")
                if entry.get("id") not in cases:
                    problems.append(f"allowlist entry {entry.get('id')!r} names no case in the set")
                allow[entry.get("id")] = entry
    disagreements = []
    families = {}
    for id_, c in cases.items():
        fam = c["family"].split("/")[0]
        families.setdefault(fam, [0, 0])
        families[fam][0] += 1
        g = got.get(id_)
        if g is None:
            problems.append(f"{id_} [{c['family']}]: no outcome in {out_path}")
            continue
        if "panic" in g:
            problems.append(f"{id_} [{c['family']}]: the walk panicked")
            continue
        ok = outcome(g) == c["expect"]
        if ok and c["expect"] is not None:
            ok = g["episodes"] == episodes_expected(c["expect"]["count"])
        if ok and c["expect"] is None:
            ok = (g["err"], g["clean_miss"]) == refusal_expected(c)
        if ok:
            continue
        families[fam][1] += 1
        disagreements.append((id_, c, g))
    accepted = 0
    for id_, c, g in disagreements:
        entry = allow.get(id_)
        line = (
            f"{id_} [{c['family']}] titles={c['titles']} count={c['count']} year={c['entry_year']} "
            f"rows={c['rows']}\n   expect={c['expect']} got={g}"
        )
        if entry is None:
            problems.append(line + "\n   not in the allowlist")
        elif entry.get("got") != summary(g):
            problems.append(line + f"\n   allowlisted, but the outcome moved: recorded {entry.get('got')}")
        else:
            accepted += 1
    disagreeing = {id_ for id_, _, _ in disagreements}
    for id_, entry in allow.items():
        if id_ in cases and id_ not in disagreeing:
            problems.append(f"allowlist entry {id_!r} is stale: the walk agrees with the model now — {entry.get('reason')}")
    print(f"pools {len(cases)} disagreements {len(disagreements)} accepted {accepted}")
    for fam, (n, d) in sorted(families.items()):
        print(f"  {fam}: {n} pools, {d} disagree")
    for p in problems:
        print(p)
    if problems:
        print(f"{len(problems)} problem(s)")
        return 1
    return 0


def main(argv):
    if len(argv) >= 3 and argv[1] == "gen":
        cases = gen()
        with open(argv[2], "w", encoding="utf-8") as fh:
            json.dump(cases, fh, ensure_ascii=False)
        print(len(cases), "pools")
        return 0
    if len(argv) >= 4 and argv[1] == "cmp":
        allowlist = None
        if len(argv) == 6 and argv[4] == "--allowlist":
            allowlist = argv[5]
        elif len(argv) != 4:
            print(__doc__, file=sys.stderr)
            return 2
        return compare(argv[2], argv[3], allowlist)
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
