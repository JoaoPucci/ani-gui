"use strict";

// How an e2e spec opens playback on a stream it serves itself, through
// the app's own playback path.
//
// A click asks the backend to play an episode (`/api/play/stream`),
// and the backend answers from its resolution cache when the user has
// turned replays on and a row for that play names a stream that still
// answers; otherwise it walks the providers, which a spec cannot reach.
// So a spec turns replays on in the profile's config (REPLAY_CONFIG),
// and, when the renderer's play request comes in, writes the row that
// play reads (replayKey, replayRow, seedReplay) and lets the request
// through. Everything past that is the shipped path: the backend checks
// the stream, opens the session, and the player fetches through the
// stream proxy.
//
// The row is written with Node's own SQLite module into the profile's
// cache, beside the running backend. What it writes — the key prefix,
// the insert, the TTL, the row's fields — is
// e2e/fixtures/replay-row.json, which the backend's tests hold to what
// it reads (backend/src/commands/play_resolution_cache_e2e_row_test.rs).
//
// Test infrastructure only; nothing in the app requires it.

const path = require("node:path");
const ROW = require("../e2e/fixtures/replay-row.json");

/** The config line that turns replays on (`Config::cache_resolutions`). */
const REPLAY_CONFIG = "cache_resolutions = true\n";

function reader(request) {
  if (request instanceof URLSearchParams) return (name) => request.get(name);
  return (name) => (request[name] === undefined ? null : request[name]);
}

/**
 * The cache key a play request reads, from what the request names:
 * `title`, `mode` and `episode`, and the `year`, `episode_count`,
 * `subtype` and `version` axes when it names them. `request` is the
 * query of a `/api/play/stream` URL or a plain object of the same
 * names.
 */
function replayKey(request) {
  const get = reader(request);
  const required = (name) => {
    const v = get(name);
    if (v === null) throw new Error(`replayKey: the play names no ${name}`);
    return String(v);
  };
  const axis = (name) => {
    const v = get(name);
    return v === null ? "-" : String(v);
  };
  const key = [
    required("title"),
    required("mode"),
    required("episode"),
    axis("year"),
    axis("episode_count"),
    axis("subtype"),
  ].join(":");
  const version = get("version");
  return `${ROW.key_prefix}${key}${version === null ? "" : `:${version}`}`;
}

/**
 * The row a replay serves: the stream at `upstreamUrl`, of
 * `mediaKind` (`"hls"` or `"mp4"`), playing `episode` (the number the
 * session names), with the sidecar `subtitles` the session lists —
 * `{ lang, label, default, url }` each, fetched by the backend.
 */
function replayRow({ upstreamUrl, mediaKind, episode = null, subtitles = [] }) {
  if (mediaKind !== "hls" && mediaKind !== "mp4") {
    throw new Error(`replayRow: no media kind ${mediaKind}`);
  }
  return {
    ...ROW.row,
    upstream_url: upstreamUrl,
    media_kind: mediaKind,
    played: episode === null ? null : String(episode),
    subtitles,
  };
}

/** The backend's metadata cache under the profile's `XDG_CACHE_HOME`. */
function metadataDb(cacheHome) {
  return path.join(cacheHome, "ani-gui", "metadata.sqlite");
}

/**
 * Writes `row` under `key` into the cache at `db`, fresh as of `now`
 * (milliseconds). The backend has the cache open; SQLite takes turns,
 * and the write waits for one.
 */
function seedReplay(db, key, row, now = Date.now()) {
  // Loaded here, not at the top: the module is Node's, and only the
  // specs that seed a row need it.
  const { DatabaseSync } = require("node:sqlite");
  const conn = new DatabaseSync(db);
  try {
    conn.exec("PRAGMA busy_timeout = 10000");
    conn
      .prepare(ROW.insert)
      .run(key, JSON.stringify(row), Math.floor(now / 1000), ROW.ttl_seconds);
  } finally {
    conn.close();
  }
}

module.exports = { REPLAY_CONFIG, metadataDb, replayKey, replayRow, seedReplay };
