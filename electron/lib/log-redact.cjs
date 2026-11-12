"use strict";

// Keeps the launch's secret out of what the main process logs.
//
// The renderer carries the secret on every API request: as the
// `x-ani-gui-internal-secret` header, and as the `secret` query
// parameter on the routes an <img> or an EventSource loads
// (frontend/src/lib/backend/secret.ts). Chromium logs a failed load with
// its URL, and the main process relays the renderer's console, its
// failed loads and the backend's output to its own stdout and stderr,
// which end up in the journal or a terminal. Everything it relays goes
// through here first.

const REDACTED = "<redacted>";

// `secret=` as a query parameter, plain or URL-encoded inside another
// URL, or standing alone at the start of a line or after a space, quote
// or bracket: the value runs to the next parameter, fragment, quote,
// bracket, separator (`;`, `,`, `|`) or whitespace. The secret is hex,
// so none of those can be part of it.
const QUERY_SECRET =
  /((?:^|[?&;\s"'(\[]|%3[Ff]|%26)secret(?:=|%3[Dd]))(?:(?!%26)[^&#\s"'<>)\];,|])*/gm;
// The header, as `name: value` or as a JSON member.
const HEADER_SECRET = /(x-ani-gui-internal-secret["']?\s*[:=]\s*["']?)[^\s"',;}\]]+/gi;
// The backend's handshake line.
const HANDSHAKE_SECRET = /(ANI_GUI_INTERNAL_SECRET\s+)\S+/g;

// A known secret shorter than this is not redacted on its own: the
// launch's is 64 hex characters, and a stray short value would blank
// ordinary words.
const MIN_KNOWN = 16;

/**
 * `text` with every secret in it replaced by `<redacted>`: each of
 * `known` (the launch's secrets, wherever they appear) and any value in
 * the shapes the secret travels in, known or not.
 */
function redactSecrets(text, known = []) {
  let out = String(text);
  for (const secret of known) {
    if (typeof secret === "string" && secret.length >= MIN_KNOWN) {
      out = out.split(secret).join(REDACTED);
    }
  }
  return out
    .replace(QUERY_SECRET, `$1${REDACTED}`)
    .replace(HEADER_SECRET, `$1${REDACTED}`)
    .replace(HANDSHAKE_SECRET, `$1${REDACTED}`);
}

/**
 * A writer for output that arrives in chunks of any size (a pipe's),
 * redacting it a whole line at a time, so a secret split across two
 * chunks is whole when it is matched. `write` gets the complete lines,
 * redacted, newlines and all; `end()` writes what is left of an
 * unfinished last line. `known` answers the secrets to redact by value
 * (redactSecrets).
 */
function redactingLines(write, known = () => []) {
  let pending = "";
  return {
    push(chunk) {
      pending += chunk;
      const cut = pending.lastIndexOf("\n");
      if (cut < 0) return;
      const lines = pending.slice(0, cut + 1);
      pending = pending.slice(cut + 1);
      write(redactSecrets(lines, known()));
    },
    end() {
      if (pending) write(redactSecrets(pending, known()));
      pending = "";
    },
  };
}

/**
 * The relays of the backend's output: `stdout(line)` for each line the
 * handshake hands on once it has completed, `stderr(chunk)` for each
 * chunk of its stderr, tagged `[backend] ` a line at a time. Both redact
 * by shape from the start, and by value the launch's secret once
 * `learn` is handed it — from the handshake, which carries it. `end()`
 * writes what is left of an unfinished stderr line.
 */
function backendRelay({ stdout, stderr }) {
  const known = [];
  const lines = redactingLines(
    (text) => stderr(text.replace(/^(?=.)/gm, "[backend] ")),
    () => known,
  );
  return {
    learn(secret) {
      if (typeof secret === "string" && secret.length > 0) known.push(secret);
    },
    stdout(line) {
      stdout(`${redactSecrets(line, known)}\n`);
    },
    stderr(chunk) {
      lines.push(chunk);
    },
    end() {
      lines.end();
    },
  };
}

module.exports = { REDACTED, backendRelay, redactSecrets, redactingLines };
