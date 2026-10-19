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
    oracle.py fuzz SEED N CASES.json
        writes N pools drawn from SEED by the differential fuzzer
        (fuzz.py), each with the pick the model predicts, for the same
        driver and the same comparison
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

def _load_alphanumeric():
    """The characters Rust's `char::is_alphanumeric` accepts, as inclusive
    ranges of code points: the letters and digits the picker's readers
    split a title's words on (alphanumeric.txt)."""
    out = []
    with open(os.path.join(HERE, "alphanumeric.txt"), encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line and not line.startswith("#"):
                a, _, b = line.partition("-")
                out.append((int(a, 16), int(b or a, 16)))
    return out


_ALNUM_RANGES = _load_alphanumeric()
_ALNUM_STARTS = [a for a, _ in _ALNUM_RANGES]

#: The Japanese blocks: the iteration and closing marks (々〆〇), the
#: Hiragana and Katakana blocks, the CJK ideographs' main block and its
#: first extension, and half-width katakana. A Japanese letter is a letter
#: of these, as the picker's `is_japanese` reads one.
_JP_BLOCKS = [(0x3005, 0x3007), (0x3040, 0x30FF), (0x3400, 0x4DBF), (0x4E00, 0x9FFF), (0xFF66, 0xFF9F)]


def _intersect(ranges, blocks):
    return [(max(a, c), min(b, d)) for a, b in ranges for c, d in blocks if max(a, c) <= min(b, d)]


def _subtract(ranges, blocks):
    out = []
    for a, b in ranges:
        pieces = [(a, b)]
        for c, d in blocks:
            pieces = [q for x, y in pieces for q in ((x, min(y, c - 1)), (max(x, d + 1), y)) if q[0] <= q[1]]
        out += pieces
    return out


def _class_body(ranges):
    return "".join(f"\\U{a:08x}" if a == b else f"\\U{a:08x}-\\U{b:08x}" for a, b in ranges)


def _alnum(c):
    """`c` is a letter or digit to the picker (`char::is_alphanumeric`)."""
    import bisect

    i = bisect.bisect_right(_ALNUM_STARTS, ord(c)) - 1
    return i >= 0 and ord(c) <= _ALNUM_RANGES[i][1]


#: The body of a character class of the Japanese letters, as the picker's
#: words hold them: ・, ゠ and the sound marks split a word, as any mark does.
JP = _class_body(_intersect(_ALNUM_RANGES, _JP_BLOCKS))

#: Rust's whitespace is Python's but for U+001C to U+001F, which Python
#: splits and strips as space and Rust's `trim` keeps. Where that decides
#: something — whether a bracket's words are the version tag, trimmed —
#: the title is read with those four as private-use marks and given back
#: after. Everywhere else they stay Python's spaces, which is how the
#: picker's word readers take them too (any character that is no letter
#: or digit splits a word); a comparison of two titles as written, which
#: Rust splits on whitespace alone, is the one place the model still
#: reads them otherwise, and no generated pool writes one there.
_RUST_MARKS = {c: 0xE000 + c - 0x1C for c in range(0x1C, 0x20)}
_PY_SPACES = {v: k for k, v in _RUST_MARKS.items()}


def _as_rust(t):
    return t.translate(_RUST_MARKS)


def _from_rust(t):
    return t.translate(_PY_SPACES)

#: The digits a title's numbers are written in: ASCII and full-width, as
#: the picker reads them. Other scripts' digits are letters to it.
DIGITS = "0123456789０１２３４５６７８９"


def _digits(s):
    """`s` is one or more digits the picker reads as digits."""
    return bool(s) and all(c in DIGITS for c in s)


#: A letter or digit outside Japanese, as the picker reads a word: any
#: Unicode letter or digit ("é", "²", "٢"), not only ASCII.
ALNUM = "[" + _class_body(_subtract(_ALNUM_RANGES, _JP_BLOCKS)) + "]"
#: A character a word is made of: a letter or digit, Japanese included.
WORD = "[" + _class_body(_ALNUM_RANGES) + "]"
#: A character no word is made of.
NOT_WORD = "[^" + _class_body(_ALNUM_RANGES) + "]"
#: What parts two words of a title: any run of characters no word is
#: made of, but for the "+" a span ("Part 1+2") is written with.
SEP = "[^" + _class_body(_ALNUM_RANGES) + "+]"
#: A word starts here / ends here: the picker's words, not Python's `\b`.
BS = "(?<!" + WORD + ")"
BE = "(?!" + WORD + ")"


def _either_width(word):
    """`word` with each character in its ASCII or its full-width form,
    each on its own: a tag is read a character at a time, so "ｔv" is
    the tag "tv" as "ｔｖ" and "tv" are."""
    return "".join(f"[{c}{chr(ord(c) + 0xFEE0)}]" for c in word)


# --- catalogue tags: "a release year or a format tag in brackets ... round,
# square or lenticular ... brackets and tag alike ASCII or full-width ...
# wherever in the title it stands", each character of the tag in either width

CTAG = re.compile(
    r"[\(\[（［【]\s*(?:"
    + "|".join(["[0-9０-９]{4}"] + [_either_width(w) for w in ("tv", "ona", "ova", "oad")])
    + r")\s*[\)\]）］】]"
)


# --- version tags: "a bracketed version tag ... in ASCII or full-width round,
# square or lenticular brackets, in any case" (Versions of one pick), each
# character of the tag ASCII or full-width, upper or lower case — and no
# other character: the picker folds a full-width form to ASCII and then
# ignores ASCII case alone, so no Unicode case folding ("ſ" for "s") applies


def _any_case_either_width(word):
    """`word` with each character in its ASCII or full-width form, in
    either case, each on its own — exactly the characters the picker reads
    as that letter, matched against the title as written."""

    def wide(c):
        return chr(ord(c) + 0xFEE0)

    return "".join(f"[{c}{c.upper()}{wide(c)}{wide(c.upper())}]" for c in word)


VTAG = re.compile(r"[\(\[（［【]\s*" + _any_case_either_width("uncensored") + r"\s*[\)\]）］】]")


def version_of(t):
    """A title read for its version: the title without its version tag,
    compared as written, and the version the tag names."""
    t = _as_rust(t)
    kind = "uncensored" if VTAG.search(t) else "standard"
    return written(_from_rust(VTAG.sub(" ", t))), kind


def _past_vtag(cand):
    return " ".join(_from_rust(VTAG.sub(" ", _as_rust(cand))).split())


def admitted_as_read(titles, cand):
    """The title rule for a probed candidate: admission read past a
    bracketed version tag, as past a catalogue tag — the tag hides no
    division before it. The probe head reads the title as written."""
    return admitted(titles, _past_vtag(cand))


def pickable_as_read(titles, cand):
    """A probed candidate the selectors choose among: `pickable`, read past
    a bracketed version tag — so "Show 1 [Uncensored]" is the entry before
    "Show 2" as "Show 1" is."""
    read = _past_vtag(cand)
    # the sequel guard drops a hit before it is probed, on its title as
    # written: the tag hides a later part from it as from the probe head
    return (
        admitted(titles, read)
        and not before_entry(titles, read)
        and not season_before_entry(titles, read)
        and not later_part_of(titles, cand)
    )


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
    if _digits(s):
        return int(s)
    return kanji_number(s)


# --- the title rule: "The entry's titles rule which candidates can be picked"

MARKER = r"(?:season|part|cour|stage)"


# Patterns built from the picker's character classes are compiled once:
# the classes are long, and a pattern rebuilt on every call is rehashed
# by the regular-expression cache on every call too.
_CARDINAL_RX = re.compile(BS + r"(" + MARKER + r")" + SEP + r"+(" + "|".join(CARD) + r")" + BE)
_WORD_ORD_RX = re.compile(BS + r"(" + "|".join(WORD_ORD) + r")" + SEP + r"+(" + MARKER + r"|half)" + BE)
_SUFFIXED_MARKER_RX = re.compile(BS + r"([0-9０-９]+)(?:st|nd|rd|th)" + SEP + "+" + MARKER)
_MARKER_NUMBER_RX = re.compile(MARKER + SEP + r"+([0-9０-９]+)(?!" + ALNUM + ")")
_NOT_WORDS_LEAD_RX = re.compile("^" + SEP + "+")
_SMALL_NUMBER_RX = re.compile(r"(?<!" + ALNUM + r")([0-9０-９]{1,2})(?!" + ALNUM + r")")
_SMALL_SUFFIXED_RX = re.compile(r"(?<!" + ALNUM + r")([0-9０-９]{1,3})(?:st|nd|rd|th)(?!" + ALNUM + r")")
_WORDS_RX = re.compile(WORD + "+")
_BARE_NUMBER_RX = re.compile(r"(?:^|" + "[^" + _class_body(_ALNUM_RANGES) + "+]" + r")([0-9０-９]{1,2})$")
_NOT_WORD_RX = re.compile(NOT_WORD)
_NOT_WORDS_RX = re.compile(NOT_WORD + "+")
_JP_BEFORE_NUMBER_RX = re.compile(r"(?<=[" + JP + r"])([0-9０-９]{1,2})$")
#: An apostrophe, however it is written: ASCII, the typographic right
#: and left single quotes, the prime, the acute accent, the grave accent
#: and the full-width apostrophe.
PRIMES = "'’‘′´`＇"
_PRIME_END_RX = re.compile(r"(?<=" + WORD + r")[" + PRIMES + r"]+(?!" + WORD + r")")
_PRIME_OPEN_RX = re.compile(r"(?<!" + WORD + r")[" + PRIMES + r"]+(?=" + WORD + r")")


def _cardinals(t):
    """A word cardinal after a marker word, one to twenty, is that ordinal."""
    return _CARDINAL_RX.sub(
        lambda m: f"{m.group(1)} {CARD[m.group(2)]}",
        norm(t),
    )


def _word_ordinals(t):
    """A word ordinal before a marker word, first to twentieth, is that
    ordinal with its suffix."""
    return _WORD_ORD_RX.sub(
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
        for m in _MARKER_NUMBER_RX.finditer(n):
            out.add(int(m.group(1)))
        for m in _SUFFIXED_MARKER_RX.finditer(n):
            out.add(int(m.group(1)))
        for m in re.finditer(r"第([0-9]+|[〇零一二三四五六七八九十百]+)", n):
            k = _jp_ordinal(m.group(1))
            if k is not None:
                out.add(k)
        for m in re.finditer(r"([0-9]+|[〇零一二三四五六七八九十百]+)[号期話部]", n):
            k = _jp_ordinal(m.group(1))
            if k is not None:
                out.add(k)
        for m in _SMALL_NUMBER_RX.finditer(n):
            out.add(int(m.group(1)))
        for m in _SMALL_SUFFIXED_RX.finditer(n):
            out.add(int(m.group(1)))
        for w in _WORDS_RX.findall(n):
            if w in ROMAN:
                out.add(ROMAN[w])
            elif w in WORD_ORD:
                out.add(WORD_ORD[w])
    return out


#: The kind of division a marker names: a season, or a part of one.
_KIND = {"season": "season", "part": "part", "cour": "part", "stage": "part", "期": "season", "部": "part", "クール": "part"}


def _tail_split(tokens):
    """Where the markers a title ends on start, among its words, and those
    markers, innermost first, each as its kind and the ordinals it names.
    The words are read once, first to last, as the reader reads them — a
    spelled or suffixed ordinal pairs with the marker after it, so "Third
    Stage 2" is a third stage and a bare 2 — and the markers are what that
    one reading ends on: a marker pair, a Japanese division, an ordinal
    spelled as one standing alone. A bare number ends no marker run (it
    stays in the stem), nor does a "Half", which is no marker word."""
    items = _division_items(tokens)
    starts, at = [], 0
    for _, width, _, _ in items:
        starts.append(at)
        at += width
    out = []
    end = len(items)
    while end > 0:
        ords, width, kind, alone = items[end - 1]
        token = tokens[starts[end - 1]]
        last = tokens[starts[end - 1] + width - 1]
        if ords is None or last == "half" or (alone and re.fullmatch(r"[0-9]+", token)):
            break
        out.append((kind, tuple(ords)))
        end -= 1
    return (starts[end] if end < len(items) else len(tokens)), out


def strip_trailing_kinds(n):
    """The markers a title ends on, innermost first, each as its kind and
    the ordinals it names, and what is left — the title's words before
    them, joined by spaces ("Show Season 3 Part 2" → "show", [part 2,
    season 3]). A span ("Part 1+2") is every part in it; brackets around a
    marker ("(Part 1+2)") are not the marker's, and marks after a title's
    last word are no word of it ("Show 2!" ends on its 2)."""
    tokens = _tokenize(norm(n))
    cut, out = _tail_split(tokens)
    return " ".join(tokens[:cut]), out


def strip_trailing(n):
    """The markers a title ends on, innermost first, as the ordinals each
    names, and what is left (`strip_trailing_kinds`)."""
    n, out = strip_trailing_kinds(n)
    return n, [span for _, span in out]


def bare_number(stem):
    """A bare number of one or two digits a stem ends on — "Show 2", the
    ２ glued to "ショー２"."""
    m = _BARE_NUMBER_RX.search(stem) or _JP_BEFORE_NUMBER_RX.search(stem)
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
    m = re.search(r"[0-9０-９]+", n)
    if not m:
        return None
    return (_NOT_WORD_RX.sub("", n[: m.start()]), m.group(0))


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
    stem, ords = strip_trailing(cand)
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
    return _NOT_WORDS_RX.sub(" ", norm(t)).strip()


SEPS = set(":-–—~,.・、：－～，．]］）)】」』")


def _tokens_end(text, nm_words):
    """The end of the shortest prefix of `text` whose cleaned words are
    `nm_words`, at a word's end (or before 第 / a glued digit)."""
    for end in range(1, len(text) + 1):
        if _clean(text[:end]).split() == nm_words:
            nxt = text[end : end + 1]
            if (
                not nxt
                or not _alnum(nxt)
                or nxt == "第"
                or (_digits(nxt) and not _digits(nm_words[-1][-1]))
            ):
                return end
    return None


def _marks(text, end):
    out = ""
    for ch in text[end:]:
        if _alnum(ch):
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
    nm = _name_tokens(_untag(t))
    if not nm:
        return False
    c, tt = _untag(cand), _untag(t)
    ce, te = _tokens_end(c, nm), _tokens_end(tt, nm)
    if ce is None or te is None or _marks(c, ce) != _marks(tt, te):
        return False
    # first divisions alone, as the reader reads them ("Season One",
    # "1st Season", "First Half", 第1期), or nothing at all
    items = _division_items(_tokenize(norm(c[ce:])))
    return all(o is not None and all(x == 1 for x in o) for o, _, _, _ in items)


def before_entry(titles, cand):
    """When every title of the entry names a later division, a candidate
    that is one of those titles' names and nothing more, or that name
    followed by first divisions alone, is the entry before it — unless it
    is one of the entry's titles or carries the number one of them does."""
    def reads(t):
        # "reads as naming a division other than the first": the reader's
        # ordinals, a division whose first ordinal is 1 left out as the
        # whole ("Part 1+2" names nothing beyond the first)
        return bool(Reading(t).ords)

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


def _name_tokens(t):
    """`t`'s name as the reader's words ("One reading of a title"): the
    words up to its first division — "怪獣８号" is one word, the ８ inside
    it no number."""
    return Reading(t).name


def _rest_after(text, own):
    """What follows `own` at the start of `text` (both the reader's words
    joined by spaces), when `own` ends there at a word's end or with a
    digit or 第 glued to it — no digit after an `own` that itself ends in
    one. An empty `own` is no stem: nothing is read past it."""
    if not own or not text.startswith(own):
        return None
    rest = text[len(own) :]
    if not rest or rest[0] == " " or rest[0] == "第" or (_digits(rest[0]) and not _digits(own[-1])):
        return rest
    return None


def _leading_number(rest):
    """The number `rest` opens on, and its digits: a run of digits ending
    the text, before a space, or before a Japanese letter glued to it."""
    m = re.match(r"\s*([0-9]+)(?=$| |[" + JP + r"])", rest)
    return (int(m.group(1)), m.group(1)) if m else None


def past_stem(cand, wide):
    """What follows `wide`'s stem in `cand`, as the reader's words, and the
    divisions `wide` carries there — the ones it ends on, and the bare
    number its stem ends on when `cand` is read past the stem without it
    ("Show Season 3" past "Show 2"). A title whose number where `wide`'s
    stands only continues it or is lower is not read past it. `None` when
    `cand` does not start with the stem."""
    wt = _tokenize(norm(wide))
    stem = _stem_tokens(wt)
    carried = [(o, k) for o, _, k, _ in _division_items(wt[len(stem) :]) if o is not None]
    text = " ".join(_tokenize(norm(cand)))
    rest = _rest_after(text, " ".join(stem))
    if rest is not None:
        return rest, carried
    if len(stem) < 2 or not re.fullmatch(r"[0-9]{1,2}", stem[-1]):
        return None
    n = int(stem[-1])
    rest = _rest_after(text, " ".join(stem[:-1]))
    if rest is None:
        return None
    lead = _leading_number(rest)
    if lead and (lead[0] < n or (lead[0] != n and lead[1].startswith(str(n)))):
        return None
    return rest, carried + [([n], "season")]


def _opening_items(rest):
    """The divisions `rest` opens with, up to the first word that names
    none, with their kinds and whether each stands alone."""
    out = []
    for item in _division_items(_tokenize(rest)):
        if item[0] is None:
            break
        out.append(item)
    return out


def _beside(cand, wide):
    """The divisions `cand` opens what follows `wide`'s stem with, as
    (ordinals, kind) — any `wide` itself ends on left out, an ordinal
    standing alone right after one of `wide`'s own seasons dividing that
    season (a part), and after a part staying a season — and the season
    `wide` carries: the highest among its own divisions, the first when
    it carries none. `None` when `cand` does not start with the stem."""
    past = past_stem(cand, wide)
    if past is None:
        return None
    rest, carried = past
    seasons = [o for ords, kind in carried if kind == "season" for o in ords]
    own = [(list(ords), kind) for ords, kind in carried]
    named = []
    after_season = False
    for ords, _, kind, alone in _opening_items(rest):
        divides = after_season and alone
        if divides:
            named.append((list(ords), "part"))
        elif (list(ords), kind) not in own:
            named.append((list(ords), kind))
        after_season = not divides and kind == "season" and (list(ords), kind) in own
    return named, (max(seasons) if seasons else 1)


def names_earlier_season(cand, wide):
    """`cand` names, past `wide`'s stem (`_beside`), a season before the
    one `wide` carries."""
    beside = _beside(cand, wide)
    if beside is None:
        return False
    named, season = beside
    return any(kind == "season" and any(o < season for o in ords) for ords, kind in named)


def _without_stem_number(t):
    """`t`'s stem without the bare number it ends on, and that number."""
    stem = _stem_tokens(_tokenize(norm(t)))
    if len(stem) >= 2 and re.fullmatch(r"[0-9]{1,2}", stem[-1]):
        return stem[:-1], int(stem[-1])
    return None


def season_before_entry(titles, cand):
    """A candidate naming a season before the one an entry title carries,
    read past that title's stem as a sibling is read past W's, or whose
    stem ends on a lower number after the same words — unless it is one
    of the entry's titles."""
    if any(written(t) == written(cand) for t in titles):
        return False
    for t in titles:
        if names_earlier_season(cand, t):
            return True
        a, b = _without_stem_number(cand), _without_stem_number(t)
        if a and b and a[0] == b[0] and a[1] < b[1]:
            return True
    return False


def part_after(title, stem):
    """The ordinal of the one short part marker — "2nd Stage", "Part 3",
    "Season 2", "II", "Second", "2nd", 第2期 — that `title` adds to
    `stem`, or `None` when `title` is not the stem followed by exactly
    one naming one ordinal: the one parse of a title's tail, read over
    what follows the stem."""
    n = norm(title)
    if not n.startswith(stem) or len(n) == len(stem):
        return None
    tail = n[len(stem) :]
    if _alnum(tail[0]) and tail[0] != "第":
        return None
    rest, ords = strip_trailing(_NOT_WORDS_LEAD_RX.sub("", tail))
    if _clean(rest) or len(ords) != 1 or len(ords[0]) != 1:
        return None
    return ords[0][0]


def later_part_of(titles, cand):
    """The sequel guard: a candidate that a title the entry goes by names a
    later part of — the candidate's title followed by one short part
    marker ("Show Season 3" for "Show Season 3 Part 2", "Lucky 2" for
    "Lucky 2 Season 2") — is dropped from the pool before probing. It is
    another entry of the same show, as the entry before is. A bare number
    is no such marker: "Show" beside "Show 2" is the entry before's shape,
    with that rule's allowance for an alias naming no later division."""
    c = norm(cand)
    return any((part_after(t, c) or 0) >= 2 for t in titles)


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

#: A word: any run of letters and digits, in any script — "推しの子IX" is
#: one word, as "Show2" is — with the "+" a span is written with.
_WORD = re.compile("[" + _class_body(_ALNUM_RANGES) + "+]+")
_JP_DIV = re.compile(r"第([0-9]+|[〇零一二三四五六七八九十百]+)(期|部|クール)")
_JP_DIV_END = re.compile(r"第([0-9]+|[〇零一二三四五六七八九十百]+)(期|部|クール)$")
_JP_DIGITS_END = re.compile(r"^(.*[" + JP + r"])([0-9]+)$")
#: A span: one- or two-digit numbers joined by "+" ("1+2"); "12+131"
#: names no span.
_SPAN = re.compile(r"^([0-9０-９]{1,2})(?:\+([0-9０-９]{1,2}))+$")


def _tokenize(n):
    """Words and Japanese divisions, in order. A span ("1+2") is one word.
    A word splits into what it holds before the Japanese divisions glued
    to its end, each 第N期 of them, and the number glued to the end of a
    Japanese letter ("ショー第2期第2部" → ショー, 第2期, 第2部; "ショー２" →
    ショー, 2; "怪獣８号２" → 怪獣8号, 2; "怪獣８号" stays one word); a
    longer number glued to a Japanese letter is a word of its own, as it
    is written apart ("ショー200" → ショー, 200). Digits glued to any other
    letter are that word's ("Show2", "推しの子IX" stay whole)."""
    out = []
    for m in _WORD.finditer(n):
        out += _apart(m.group(0))
    return out


def _apart(w):
    """One word read apart: the Japanese divisions glued to its end, and
    a number glued to the end of a Japanese letter — a division's last
    letter included ("第3期２" is 第3期 and 2) — each a word of its own."""
    divs = []
    while True:
        d = _JP_DIV_END.search(w)
        if not d:
            break
        divs.insert(0, d.group(0))
        w = w[: d.start()]
    if not w:
        return divs
    g = _JP_DIGITS_END.match(w)
    if not g:
        return [w] + divs
    return _apart(g.group(1)) + [g.group(2)] + divs


def _is_marker(w):
    return w in ("season", "part", "cour", "stage")


def _ordinal_of(w):
    """A word read as an ordinal where one may stand alone: a suffixed
    number, a spelled ordinal, a Roman numeral, a bare number of one or
    two digits (a zero included), a Japanese division."""
    m = re.fullmatch(r"([0-9０-９]{1,3})(?:st|nd|rd|th)", w)
    if m:
        return [int(m.group(1))]
    if w in WORD_ORD:
        return [WORD_ORD[w]]
    if w in ROMAN:
        return [ROMAN[w]]
    if re.fullmatch(r"[0-9０-９]{1,2}", w):
        return [int(w)]
    m = _JP_DIV.fullmatch(w)
    if m:
        k = _jp_ordinal(m.group(1))
        return None if k is None else [k]
    return None


def _divisions(tokens):
    """Each token read as a word (`None`) or as the ordinals of the
    division it opens, with the tokens the division spans."""
    return [(ords, width) for ords, width, _, _ in _division_items(tokens)]


def _division_items(tokens):
    """`_divisions`, each division with the kind it names — a season, or
    a part (a part, a cour, a stage, 第N部, 第Nクール) — and whether it is
    an ordinal standing alone: a Roman numeral, a word ordinal, a suffixed
    or bare number, an ordinal before "Half". Standing alone, an ordinal
    names a season."""
    items = []
    i = 0
    while i < len(tokens):
        w = tokens[i]
        nxt = tokens[i + 1] if i + 1 < len(tokens) else None
        if _is_marker(w) and nxt is not None:
            kind = _KIND[w]
            if _SPAN.fullmatch(nxt):
                items.append(([int(k) for k in nxt.split("+")], 2, kind, False))
                i += 2
                continue
            if _digits(nxt) and len(nxt) <= 3:
                items.append(([int(nxt)], 2, kind, False))
                i += 2
                continue
            if nxt in CARD:
                items.append(([CARD[nxt]], 2, kind, False))
                i += 2
                continue
            # the ordinal after a marker word, spelled, suffixed or Roman
            # ("Season Second", "Season 2nd", "Season II")
            if re.fullmatch(r"[0-9０-９]{1,3}(?:st|nd|rd|th)", nxt) or nxt in WORD_ORD or nxt in ROMAN:
                items.append((_ordinal_of(nxt), 2, kind, False))
                i += 2
                continue
        if (re.fullmatch(r"[0-9０-９]{1,3}(?:st|nd|rd|th)", w) or w in WORD_ORD) and nxt is not None and (
            _is_marker(nxt) or nxt == "half"
        ):
            if nxt == "half":
                items.append((_ordinal_of(w), 2, "season", True))
            else:
                items.append((_ordinal_of(w), 2, _KIND[nxt], False))
            i += 2
            continue
        if w in WORD_ORD:
            # a word ordinal standing alone ends the name only where
            # nothing but divisions follows it
            if all(o is not None for o, _, _, _ in _division_items(tokens[i + 1 :])):
                items.append(([WORD_ORD[w]], 1, "season", True))
            else:
                items.append((None, 1, None, False))
            i += 1
            continue
        if w == "x":
            # X is never a numeral, being a word of its own in titles like "Spy x Family"
            items.append((None, 1, None, False))
            i += 1
            continue
        k = _ordinal_of(w)
        if k is not None and not re.fullmatch(r"[0-9０-９]{3,}", w):
            m = _JP_DIV.fullmatch(w)
            if m:
                items.append((k, 1, _KIND[m.group(2)], False))
            else:
                items.append((k, 1, "season", True))
        else:
            items.append((None, 1, None, False))
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
                    self.ords = _flat(_ordinals_of(_divisions(stem)))
                    self.ords += _flat(_ordinals_of(tail_items))
                    j = len(stem)
                    for t_ords, t_width in tail_items:
                        if t_ords is None:
                            self.rest.append(tokens[j])
                        j += t_width
                    break
                in_name = False
                if ords[0] != 1:
                    # the ordinals a division names, a span's each in turn
                    self.ords += ords
            i += width
        low = title.lower()
        tags = [_ascii(m.group(0))[1:-1].strip() for m in CTAG.finditer(low)]
        years = [t for t in tags if _digits(t)]
        self.year = years[0] if years else None
        # each run of apostrophes counts by its length
        ends = sum(len(m) for m in _PRIME_END_RX.findall(low))
        opens = sum(len(m) for m in _PRIME_OPEN_RX.findall(low))
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


def _flat(ordinals):
    """`_ordinals_of`'s divisions as the ordinals they name, in order —
    a span's each in turn — the whole ones left out."""
    out = []
    for o in ordinals:
        if o is not None:
            out += list(o) if isinstance(o, tuple) else [o]
    return out


def _ordinals_of(items):
    out = []
    for ords, _ in items:
        if ords is None:
            continue
        out.append(None if ords[0] == 1 else (ords[0] if len(ords) == 1 else tuple(ords)))
    return out


def _stem_tokens(tokens):
    """The title with every marker and Japanese division it ends on
    removed — an ordinal spelled as one standing alone ("Show Seventh",
    "Show 20th") is one — while a bare or glued number it ends on
    stays (`_tail_split`)."""
    return tokens[: _tail_split(tokens)[0]]


def names(titles, cand):
    """The entry names a title when one of its titles carries the title's
    name, reads as it does, carries the same words past that name, the
    same primes and no conflicting release year — under any alias."""
    rc = Reading(cand)
    return any(Reading(t).same_show(rc) for t in titles)


# --- choosing among candidates

def parts_of(t):
    """Every ordinal the part markers name in the run of markers a title
    ends on — a part, a cour or a stage, as the kind of division a marker
    word names; none reads as the first part."""
    _, out = strip_trailing_kinds(t)
    return {k for kind, span in out if kind == "part" for k in span}


def part_agrees(titles, cand):
    """Part agreement never decides which hits the probe head takes, and
    reads past a bracketed version tag, which hides no part."""
    eparts = set().union(*(parts_of(t) for t in titles)) or {1}
    parts = parts_of(_past_vtag(cand)) or {1}
    # a span without the first part agrees only where the entry ends on
    # every part in it, as the title rule admits one
    return parts <= eparts if 1 not in parts else bool(parts & eparts)


def probed(titles, rows, counted):
    """Every hit the probe head takes, in the provider's order: the five
    that could be picked, as written, and with a count the evidence
    beside them. Once probed, a hit is read past its version tag, so
    evidence the tag made unpickable as written can be picked as read."""
    picked, evidence = head(titles, rows, counted)
    taken = {id(r) for r in picked + evidence}
    return [r for r in rows if id(r) in taken]


def head(titles, rows, counted):
    """The probe head: at most five hits that could be picked and, with a
    count, at most five more as evidence — the first five that could not
    be picked, up to the fifth that could."""
    picked, evidence = [], []
    for r in rows:
        if later_part_of(titles, r[1]):
            # the sequel guard drops it from the pool before probing: it is
            # neither considered nor evidence
            continue
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
    return bool(rest) and (rest[0] == "第" or (_digits(rest[0]) and not _digits(last[-1])))


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
    adm = [r for r in probed(titles, rows, True) if pickable_as_read(titles, r[1])]
    if count_stands_alone(count, entry_year):
        adm = [r for r in adm if vouched_by_name(titles, searched, r[1])]
    if needs_a_name(count, entry_year):
        adm = [r for r in adm if vouched_dated(titles, searched, r[1], names_of(r[0]))]
    tol = max(3, count // 10)
    fits = [r for r in adm if abs(r[2] - count) <= tol]
    if not fits:
        # the airing rescue: a candidate whose own year is the entry's,
        # listing fewer episodes than the entry has, is an airing part —
        # by distance, then the exact title, then part agreement — and
        # one pick, never with versions
        short = [r for r in adm if entry_year == LISTING_YEAR and r[2] < count]
        if not short:
            return None, []
        best = min(
            short,
            key=lambda r: (
                count - r[2],
                0 if written(r[1]) == written(searched) else 1,
                0 if part_agrees(titles, r[1]) else 1,
            ),
        )
        return best, [best]
    best = min(abs(r[2] - count) for r in fits)
    tied = [r for r in fits if abs(r[2] - count) == best]
    # beside a broad listing W of the entry's year, an O — here, one of
    # the entry's own titles, read past its tag, listing exactly E —
    # scores every other exact fit one behind it, whatever W is
    broad = entry_year == LISTING_YEAR and any(r[2] > count and admitted_as_read(titles, r[1]) for r in rows)
    own = {written(t) for t in titles}

    def is_o(r):
        return broad and r[2] == count and written(_past_vtag(r[1])) in own

    return sorted(
        tied,
        key=lambda r: (
            0 if is_o(r) else 1,
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
    return strip_trailing(t)[0]


def _opens_past_stem(title, stem):
    """`title` starts with `stem` at a word's end, or with a digit or 第
    glued to it (no digit when the stem itself ends in one)."""
    n = norm(title)
    if not stem:
        # an empty stem is no stem: nothing is read past it
        return False
    if not n.startswith(stem) or len(n) == len(stem):
        return n == stem
    nxt = n[len(stem)]
    if nxt == "第" or not _alnum(nxt):
        return True
    return _digits(nxt) and not _digits(stem[-1])


def _cut_kin(r, w, count):
    """Whether the sibling `r` of the broad listing `w` is one the cut
    reads: an L or an O by its count, or by its divisions an F (first
    divisions alone past W's stem) or a P (a season before W's)."""
    if r[2] in (count, w[2] - count):
        return True
    a, b = _past_vtag(r[1]), _past_vtag(w[1])
    beside = _beside(a, b)
    if beside is None:
        return False
    # first divisions alone: each division's first ordinal is 1, a span
    # through the first ("Part 1+2") reading as the whole
    first = bool(beside[0]) and all(ords[0] == 1 for ords, _ in beside[0])
    return first or names_earlier_season(a, b)


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
            short = re.sub(r"\s*(?<![0-9０-９])[0-9０-９]{1,2}$", "", stem)
            stems = {stem, short} if short.strip() else {stem}
            kin = [r for r in rows if r is not w and any(_opens_past_stem(r[1], s) for s in stems)]
            # the spanning cut reads a probed candidate past its version
            # tag, so a tagged sibling is kin as its plain copy is
            kin += [r for r in rows if r is not w and past_stem(_past_vtag(r[1]), _past_vtag(w[1])) is not None]
            # a broad listing a sibling could complete or bar: the spanning
            # cut, which takes an L (W's count less the entry's) or an O
            # (the entry's count) and never picks an F or a P, whatever
            # their counts
            if w[2] > count and admitted_as_read(titles, w[1]) and any(_cut_kin(r, w, count) for r in kin):
                return NOT_MODELLED
            # an admitted bare title beside later parts of it — the title
            # followed by one short part marker — whose counts could
            # together explain the entry's, within the tolerance or short
            # of it: stitching. A refused lead heads no chain, and a first
            # part completes none.
            parts = [
                r
                for r in rows
                if r is not w
                and max(
                    part_after(r[1], norm(w[1])) or 0,
                    part_after(_past_vtag(r[1]), norm(_past_vtag(w[1]))) or 0,
                )
                >= 2
            ]
            if admitted_as_read(titles, w[1]) and parts and w[2] + min(r[2] for r in parts) <= count + tol:
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
        # a hit the sequel guard drops is another entry, no refusal
        return not late and not any(not admitted(titles, r[1]) and not later_part_of(titles, r[1]) for r in rows)
    taken, evidence = head(titles, rows, True)
    tol = max(3, count // 10)
    # evidence is probed too, and read past its version tag once probed
    if any(abs(r[2] - count) <= tol and not admitted_as_read(titles, r[1]) for r in evidence):
        return False
    if any(abs(r[2] - count) <= tol and not admitted_as_read(titles, r[1]) for r in taken):
        return False
    # the count-alone refusal is a refusal by title too: a fitting hit
    # nothing names, among the hits probed, leaves no clean miss
    if count_stands_alone(count, case["entry_year"]):
        adm = [r for r in probed(titles, rows, True) if pickable_as_read(titles, r[1])]
        return not any(abs(r[2] - count) <= tol for r in adm)
    # so is the dated-name refusal: a fitting hit no name vouches for —
    # under any title searched, since every title's search answers with
    # the same rows here and the walk reads every pool
    # — any hit the year and count would have taken: within the
    # tolerance, or from the entry's year (every listing's here) and
    # shorter than the entry, as the airing rescue and the stitching take
    if needs_a_name(count, case["entry_year"]):
        adm = [r for r in probed(titles, rows, True) if pickable_as_read(titles, r[1])]
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
    m = re.search(r"[0-9０-９]+", n)
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
            # "Lucky 2 2nd Season" reads [2, 2]: L. So does "ショー２第2期": the ２ glued
            # to the end of a Japanese word stands as a number before the division
            # glued to it — not inside a word, as 怪獣８号's ８ is — so it reads
            # [2, 2] and opens what follows W's stem with a later season: L.
            shapes.append((t + ("第2期" if jp else " 2nd Season"), "cut"))
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
            m = re.search(r"[0-9０-９]+$", t)
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
        # a closing bracket around the name, in either width
        ("［Oshi no Ko］ 2nd Season", "Oshi no Ko"),
        ("［Show］ 2", "Show"),
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
        ("Show (ｔv)", None),
        ("Show Season 1 （２0２0）", None),
        ("Show ［ＴＶ］", None),
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
        # a tag mixing widths is read a character at a time: the same tag
        ("Fruits Basket (2019)", 25, [("old", "Fruits Basket", 25), ("new", "Fruits Basket (２0１9)", 25)], "new"),
        ("Show (ｔv)", E, [("own", "Show (TV)", E), ("other", "Other", E)], "own"),
        ("Show (TV)", E, [("other", "Other", E), ("own", "Show （Ｔv）", E)], "own"),
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
        vtag = ("(Uncensored)", "[UNCENSORED]", "（Uncensored）", "(ｕncensored)", "［ＵＮＣＥＮsored］")[i % 5]
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
    # a broad listing whose stem is empty is cut by nothing: nothing is read
    # past an empty stem
    for wide, sib in [("Season 2", "3rd Season"), ("Season 2", "3"), ("Season 3", "Season 3 Part 2")]:
        rows = [("w", wide, 22), ("s", sib, 10)]
        case = dict(titles=[wide], count=E, entry_year=LISTING_YEAR, rows=[list(r) for r in rows])
        want = predict(case)
        if want is not NOT_MODELLED:
            add([wide], E, LISTING_YEAR, rows, want, "empty-stem")
    # a version tag is its letters in ASCII or full-width and in either
    # case, and nothing else: "ſ" is no "s" to the picker, so "(Uncenſored)"
    # is bracketed words, as "(Uncensor)" is
    for entry, cnt, rows in [
        ("Show", E, [("p", "Show", E), ("u", "Show (Uncenſored)", E)]),
        ("Show", E, [("u", "Show (Uncenſored)", E), ("p", "Show", E)]),
        ("Show 2", E, [("x", "Show 1 (Uncenſored)", E)]),
        ("Show 2", None, [("x", "Show 1 (Uncenſored)", E)]),
        ("Show", E, [("x", "Show Season 2 (Uncenſored)", E)]),
        ("Show 2", E, [("x", "Show 1 (ｕncensor)", E)]),
        # U+001F is a mark to the picker, not a space inside the bracket
        ("Show", E, [("p", "Show", E), ("u", "Show (\x1fUncensored)", E)]),
    ]:
        case = dict(titles=[entry], count=cnt, entry_year=LISTING_YEAR, rows=[list(r) for r in rows])
        add([entry], cnt, LISTING_YEAR, rows, predict(case), "not-a-tag")
    # a word is any run of letters and digits, in any script; a number is
    # written in ASCII or full-width digits, so another script's digits or
    # a superscript are a word's letters, no number, to every reader
    for entry, cand in [
        ("Pokémon", "Pokémon 2"),
        ("Pokémon 2", "Pokémon"),
        ("Pokémon 2", "Pokémon Season 1"),
        ("Pokémon", "Pokemon"),
        # a name is read past at a word's end, or before a glued digit or 第
        ("Show 2", "Showtime Season 1"),
        ("Show 3", "Show2"),
        ("Show 3", "Show2 Part 2"),
        ("Show 2", "Show 1 Shin"),
        ("Show 2 Season 3", "Show 1 Shin Season 1"),
        # a letter is what the picker's words hold: a combining mark of a
        # letter is one, ・ is none, and a Roman numeral is a whole word
        ("Show 2", "Showा Season 1"),
        ("ショー・2", "ショー"),
        ("Show Èv", "Show Season 5"),
        ("Show éii", "Show Season 2"),
        # a number glued to a word is a number only after a Japanese letter
        ("ＳＨＯＷ３", "ＳＨＯＷ２"),
        ("ＳＨＯＷ", "ＳＨＯＷ２"),
        ("Show 3", "ＳＨＯＷ２"),
        ("Café3", "Café2"),
        ("Café3", "Café"),
        ("Шоу3", "Шоу2"),
        ("나혼자만레벨업3", "나혼자만레벨업2"),
        ("Show²3", "Show²2"),
        ("Show 3", "Show ²2"),
        ("ｼｮｰ3", "ｼｮｰ2"),
        ("時々3", "時々2"),
        # digits standing before a letter glued to them, in a stem's place
        ("Show 2", "Show 20é Season 1"),
        ("Show 2", "Show 1é Season 1"),
        ("Show 2", "Show 3é"),
        ("Show 2", "Show 20第1期"),
        ("ショー２", "ショー２０第1期"),
        # a title that is nothing but a division has an empty name
        ("Season 2", "1st Season"),
        ("Season 2", "Season 1"),
        ("Season 3", "2nd Season"),
        ("Season 2", "First Half"),
        ("Season 2", "Second Half"),
        # an entry title opening on a mark before an empty name
        ("・第2期", "・第1期"),
        ("- Season 2", "- Season 1"),
        (": Season 2", ": Season 1"),
        ("- Season 2", "- 1st Season"),
        # U+001C to U+001F end a title as any mark does
        ("Show", "Show 2\x1e"),
        ("Show 3", "Show 2\x1e"),
        ("Show", "Season 2\x1f"),
        ("Season 2", "Season 2 Part 2"),
        ("Season 2", "Other Show"),
        ("Show", "Show ٢"),
        ("Show", "Show Season ٢"),
        ("Show", "Show ۲ Part ٢"),
        ("Show", "Show ٢٠٢٠"),
        ("Show", "Show (٢٠٢٠)"),
        ("Show", "Show²"),
        ("Show ٢", "Show"),
        ("Show ٢", "Show ٢"),
        ("Show 2", "Show ١"),
        ("Show 2", "Show² Season 1"),
        ("ショー", "ショー٢"),
    ]:
        for cnt in (E, None):
            rows = [("x", cand, E)]
            case = dict(titles=[entry], count=cnt, entry_year=LISTING_YEAR, rows=[list(r) for r in rows])
            add([entry], cnt, LISTING_YEAR, rows, predict(case), "unicode-words")
    # a version tag inside other bracketed words is the tag: a bracket that
    # holds no tag is read past its opener (Versions of one pick)
    for plain in ("Show (Shin", "Show [Extra", "Show （Ｘ"):
        for tag in ("(Uncensored)", "［ＵＮＣＥＮsored］"):
            copy = ("u", f"{plain} {tag}", E)
            for rows in ([("p", plain, E), copy], [copy, ("p", plain, E)]):
                case = dict(titles=[plain], count=E, entry_year=LISTING_YEAR, rows=[list(r) for r in rows])
                add([plain], E, LISTING_YEAR, rows, predict(case), "nested-tag")
    # every reader reads a title without its catalogue tags — every family again, tagged
    tags = ["(TV)", "(2024)", "[OVA]", "（ＯＮＡ）", "【OAD】", "（２０２０）", "(ｔv)", "（２0２0）", "[Ｏva]", "［ＯＶＡ］", "［2020］"]
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
    if len(argv) == 5 and argv[1] == "fuzz":
        # run with -I, the script's directory is not on the path
        sys.path.insert(0, HERE)
        sys.modules.setdefault("oracle", sys.modules[__name__])
        import fuzz

        cases = fuzz.fuzz(int(argv[2]), int(argv[3]))
        with open(argv[4], "w", encoding="utf-8") as fh:
            json.dump(cases, fh, ensure_ascii=False)
        print(len(cases), "pools from seed", argv[2])
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
