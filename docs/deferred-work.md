

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

**These entries were carried across from session state and have proven
unreliable about their own status.** Nine were already shipped, every
one caught in review of this change rather than by the person who
wrote the list. Confirm an item against the code before starting it,
and delete it here when you find it done — the list earns trust by
being pruned, not by being long.

Nine out of roughly thirty is not a list with a few stale rows; it is
a list whose status field means nothing yet. A full audit against the
code is worth doing before anyone plans from it. Until that happens,
treat every entry as a lead rather than a fact.

## Resolver and provider

- **Replace the bundled `ani-cli` with a native Rust resolver.** Search
  and episode-count disambiguation are already native —
  `scraper/allanime.rs` stops there. Two things remain only in the
  script, and both are needed before playback works natively:

  1. **Key derivation and source decryption**, which is what the
     provider's change just broke.
  2. **Turning a decrypted `sourceUrl` into a playable stream** —
     `generate_link` / `get_links`: provider embed requests,
     master-playlist expansion, quality selection, referer selection.

  Doing only the first produces correct ciphertext handling and still
  no URL to play. Retires the carried patch set in `AGENTS.md` §3 by
  deleting the script, but only once both exist.
- **The provider's crypto flow changed and playback is broken.**
  Upstream has two competing unmerged fixes. The smaller one
  identifies the real inputs — a lane parameter, build id, locally
  generated mask and epoch feeding a bootstrap header — which is a
  pure function and therefore testable against captured fixtures
  without a subprocess. That makes it the smallest native slice worth
  taking first.
- **Validate the botan wrapper on a packaged Windows build** under Git
  Bash.
- **JSON-escape `$1` in `search_anime`** — a carried fork patch that
  was never written.

## Correctness in the app

- **Distinguish "no sources upstream" from "show not found"** in the
  play error path. They are the same message today and want different
  advice.
- **Check the Yani Neko Mini situation.**

## Testing and CI

- **The CRAP ratchet disagrees between CI and local** — 26 against 25 —
  and three files sit at 29.7–30.0, right on the high-risk boundary.
- **The pre-commit hook and strict TDD are in tension.** A `test(red):`
  commit is failing by construction and the hook rejects it, so the
  discipline and the tooling contradict each other. This has been
  worked around with `--no-verify`, which disables every other check
  the hook performs — including the ones that would have caught a
  failing suite. Reconciling them is the fix; the workaround is not.

## Interface

- **Localised content fetch** — synopsis and episode titles.
- **Franchise and season grouping** across surfaces.
- **Play-page keep-alive → normal reload with a persisted position.**
- **Search has no sort *direction* control.** Sorting by relevance,
  title, year and rating ships, as do subtype filter chips; only
  ascending/descending is absent. Name any further filters wanted
  before starting, rather than reading this as filters being missing.
- **Update notifier is not resilient to GitHub rate limits.**
- **Document Picture-in-Picture** — blocked upstream on
  electron/electron#39633, open since 2023. Do not re-attempt until it
  lands.
- **Illustrated brand assets** — post-1.0.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
