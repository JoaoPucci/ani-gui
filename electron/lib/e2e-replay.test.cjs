// The e2e specs open playback through the app's own path: the backend
// replays a row they seed into its resolution cache. The key a play
// reads and the row it serves are the fixture's, which the backend's
// tests hold to the cache (play_resolution_cache_e2e_row_test.rs); this
// holds the helper to the fixture.

const test = require("node:test");
const assert = require("node:assert/strict");
const fc = require("fast-check");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const ROW = require("../e2e/fixtures/replay-row.json");
const { REPLAY_CONFIG, metadataDb, replayKey, replayRow, seedReplay } = require("./e2e-replay.cjs");

test("a play's key is the fixture's, from an object or a query", () => {
  const { request, key } = ROW.sample;
  assert.equal(replayKey(request), key);
  assert.equal(replayKey(new URLSearchParams(request)), key);
});

test("a play naming a version and no axes keys as the fixture says", () => {
  const { request, key } = ROW.sample.versioned;
  assert.equal(replayKey(request), key);
  assert.equal(replayKey(new URLSearchParams(request)), key);
});

test("what a play's query does not carry plays no part in its key", () => {
  const { request, key } = ROW.sample;
  const query = new URLSearchParams({ ...request, prefetch: "1", kitsu_id: "1", alt_titles: "a\nb" });
  assert.equal(replayKey(query), key);
});

test("a play naming no title, mode or episode has no key", () => {
  for (const name of ["title", "mode", "episode"]) {
    const request = { ...ROW.sample.request };
    delete request[name];
    assert.throws(() => replayKey(request), new RegExp(name));
  }
});

test("a key starts with the play family and ends with the episode's axes", () => {
  const word = fc.stringMatching(/^[A-Za-z0-9 ]{1,12}$/);
  fc.assert(
    fc.property(word, fc.constantFrom("sub", "dub"), fc.integer({ min: 1, max: 999 }), (title, mode, ep) => {
      const key = replayKey({ title, mode, episode: String(ep) });
      assert.equal(key, `${ROW.key_prefix}${title}:${mode}:${ep}:-:-:-`);
    }),
  );
});

test("a row is the fixture's with the stream, kind, episode and tracks the spec names", () => {
  const subtitles = [{ lang: "en", label: "English", default: true, url: "http://127.0.0.1:9/en.vtt" }];
  const row = replayRow({ upstreamUrl: "http://127.0.0.1:9/clip.mp4", mediaKind: "mp4", episode: 6, subtitles });
  assert.deepEqual(row, {
    ...ROW.row,
    upstream_url: "http://127.0.0.1:9/clip.mp4",
    media_kind: "mp4",
    played: "6",
    subtitles,
  });
  assert.equal(replayRow({ upstreamUrl: "u", mediaKind: "hls" }).played, null);
  assert.throws(() => replayRow({ upstreamUrl: "u", mediaKind: "Hls" }), /media kind/);
});

test("replays are turned on by the config's own key", () => {
  assert.equal(REPLAY_CONFIG, "cache_resolutions = true\n");
});

test("the cache is the profile's metadata database", () => {
  assert.equal(metadataDb("/p/cache"), path.join("/p/cache", "ani-gui", "metadata.sqlite"));
});

test("a seeded row is written under its key, fresh, for the play TTL", (t) => {
  const { DatabaseSync } = require("node:sqlite");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-replay-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const db = path.join(dir, "metadata.sqlite");
  // The table as the backend's first migration makes it; the backend's
  // own test runs the insert against its real schema.
  const setup = new DatabaseSync(db);
  setup.exec(
    "CREATE TABLE meta_cache (key TEXT PRIMARY KEY, body TEXT NOT NULL, fetched_at INTEGER NOT NULL, ttl_seconds INTEGER NOT NULL)",
  );
  setup.close();

  const row = replayRow({ upstreamUrl: "http://127.0.0.1:9/master.m3u8", mediaKind: "hls", episode: 1 });
  seedReplay(db, "k", { stale: true }, 1_000);
  seedReplay(db, "k", row, 1_700_000_000_999);

  const read = new DatabaseSync(db);
  try {
    const rows = read.prepare("SELECT key, body, fetched_at, ttl_seconds FROM meta_cache").all();
    assert.equal(rows.length, 1, "a second seed replaces the first");
    assert.equal(rows[0].key, "k");
    assert.deepEqual(JSON.parse(rows[0].body), row);
    assert.equal(rows[0].fetched_at, 1_700_000_000);
    assert.equal(rows[0].ttl_seconds, ROW.ttl_seconds);
  } finally {
    read.close();
  }
});
