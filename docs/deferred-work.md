

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

## Resolver and provider

- **Replace the bundled `ani-cli` with a native Rust resolver.** The
  search and episode-count disambiguation are already native; what the
  script still uniquely does is key derivation and source decryption.
  Retires the whole carried patch set in `AGENTS.md` §3 by deleting
  the script.
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
- **Episode availability should be correct on arrival**, rather than
  corrected after a probe settles.
- **Re-probe availability when a dimmed aired-but-uncatalogued tile is
  clicked.**
- **Retry gate-refused continue-watching resolves** once the circuit
  breaker recovers.
- **Continue Watching: the Meitantei Conan row orphans to /search** —
  the reverse resolve fails for it.
- **Search holds results behind the strict availability probe**; it
  should render and prune.
- **Check the Yani Neko Mini situation.**

## Testing and CI

- **`api_play` is not hermetic** — it hits live allanime, so local runs
  fail on IP throttling and only CI is authoritative. Stub the
  Rust-side search.
- **Frontend acceptance infrastructure** (MSW plus route mounting, or
  Playwright) and the first scenarios.
- **Harden the Playwright cold-launch** against worker teardown.
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
- **Custom frameless titlebar** with OS-layout-aware window controls.
- **Search filters** — sort direction and filter options.
- **Update notifier is not resilient to GitHub rate limits.**
- **Document Picture-in-Picture** — blocked upstream on
  electron/electron#39633, open since 2023. Do not re-attempt until it
  lands.
- **Illustrated brand assets** — post-1.0.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
