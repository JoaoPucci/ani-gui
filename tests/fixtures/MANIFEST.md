# Fixture manifest

Every fixture under `tests/fixtures/` is a recorded response or sample input
used by tests across every layer (Bash, Rust, TypeScript). Each subdirectory
has its own `MANIFEST.json` describing each file's source URL, capture
timestamp, and SHA-256. Fixtures over 1 MB live in git-LFS.

## Subdirectories (populated as tests are added)

| Path | Contents |
|---|---|
| `anidb/` | Synthesized anidb.app response shapes the native resolver scrapes: browse pages (results, empty, cloudflare interstitial), a detail page (MAL link + Seasons), episodes JSON, languages JSON, an embed page, and a master playlist. |
| `kitsu/` | JSON:API responses for `/anime?filter[text]=`, `/anime/:id`, `/anime?filter[status]=`, `/anime/:id/relationships/genres`. |
| `history/` | Watch-history samples for the GUI's reader: empty, single-entry, multi-entry, duplicate-id, malformed-line. |
| `hls/` | Synthesized MPEG-TS samples for the player's demuxer regression tests, as base64 text: a stream whose video starts under one second and whose audio starts after it. |
| `arch/` | Stand-in checks the bats harness under `tests/bash/arch/` drives: a check a stray environment can redirect. |

## Refresh flow

```
make fixtures-refresh        # re-records against live APIs, writes diff report
git diff tests/fixtures/     # human-reviewed in PR
```

The refresh target writes a per-subdirectory `MANIFEST.json` update with new
SHA-256s. Reviewers should look for *unexpected* diffs (e.g. a Kitsu response
that gained a new field — investigate before accepting), and for changes in
fixtures that property tests depend on.

## Capture conventions

- Responses are captured with the same User-Agent the production code uses.
- For anidb.app, capture through the same impersonating transport the
  resolver spawns — a plain client is answered with the cloudflare
  interstitial, which is itself one of the recorded shapes.
- Personally-identifying fields (none expected for these APIs, but worth
  checking) are scrubbed before commit.
- Binary fixtures (the `tobeparsed` blobs) are committed as base64 text in
  `.b64` files so a reviewer can `git diff` them.
