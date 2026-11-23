"use strict";

// What a launched app said, kept for the report of an e2e test that
// fails.
//
// A failing spec otherwise reports only what Playwright saw: an
// assertion, or an error from its driver such as "Execution context
// was destroyed", which is all an app that quit on its own leaves
// behind. The app's own lines — the main process's `[main]` record of
// the boot, the backend's log, a refusal — and how it ended (an exit
// code, a signal) are what say why, and the suite printed none of
// them. The shared launch helper (e2e/launch.ts) feeds every launch
// made inside a test here, and the suite's fixture (e2e/policy-guard.ts)
// reports the record of a test that did not end as expected.
//
// Only the last `maxLines` lines are kept, each cut at `maxLineChars`,
// so a long test's backend log cannot crowd the report out; the record
// says what went. Pure: it is fed chunks and an ending, and reads
// nothing itself.

const { StringDecoder } = require("node:string_decoder");

/** How many lines a record keeps, by default. */
const MAX_LINES = 1000;

/** How many characters of one line a record keeps, by default. */
const MAX_LINE_CHARS = 4000;

/**
 * A recorder for one launch. `label` names it in the report (the
 * launch's pid, say). Returns:
 *
 *   feed(stream, chunk)  a chunk ("stdout" or "stderr"; a string or a
 *                        Buffer) of what that stream carried
 *   end(stream)          the stream closed: a last line with no
 *                        newline after it is kept
 *   exited(code, signal) how the process ended, as child_process's
 *                        `exit` event reports it
 *   failed(err)          the process never started (the spawn's error)
 *   text({ last })       the record, as the report shows it; with
 *                        `last`, only that many of its last lines
 */
function outputRecorder(label, { maxLines = MAX_LINES, maxLineChars = MAX_LINE_CHARS } = {}) {
  const lines = [];
  let dropped = 0;
  let ending = null;
  // Per stream: the line still waiting for its newline, and how many of
  // its characters went past maxLineChars.
  const pending = new Map();
  const decoders = new Map();

  const shown = (stream, line, cut) =>
    `[${stream}] ${line}${cut > 0 ? ` … (${cut} more characters not kept)` : ""}`;
  const keep = (stream, { line, cut }) => {
    lines.push(shown(stream, line, cut));
    if (lines.length > maxLines) {
      lines.shift();
      dropped += 1;
    }
  };
  const decoderOf = (stream) => {
    if (!decoders.has(stream)) decoders.set(stream, new StringDecoder("utf8"));
    return decoders.get(stream);
  };
  // `text` joined to what `stream` has waiting, bounded; nothing is
  // left waiting.
  const take = (stream, text) => {
    const was = pending.get(stream) ?? { line: "", cut: 0 };
    pending.delete(stream);
    const line = was.line + text;
    if (line.length <= maxLineChars) return { line, cut: was.cut };
    return { line: line.slice(0, maxLineChars), cut: was.cut + line.length - maxLineChars };
  };

  return {
    feed(stream, chunk) {
      // A Buffer may end inside a multi-byte character: the decoder
      // keeps its first bytes until the rest arrive.
      const text = typeof chunk === "string" ? chunk : decoderOf(stream).write(chunk);
      const parts = text.split("\n");
      const rest = parts.pop();
      for (const part of parts) keep(stream, take(stream, part));
      pending.set(stream, take(stream, rest));
    },
    end(stream) {
      const tail = decoders.has(stream) ? decoders.get(stream).end() : "";
      const whole = take(stream, tail);
      if (whole.line.length > 0 || whole.cut > 0) keep(stream, whole);
    },
    exited(code, signal) {
      ending = signal ? `ended by ${signal}` : `exited with code ${code}`;
    },
    failed(err) {
      ending = `did not start (${(err && err.message) || err})`;
    },
    text({ last } = {}) {
      // A line still waiting for its newline is shown as it stands.
      const partial = [...pending]
        .filter(([, p]) => p.line.length > 0 || p.cut > 0)
        .map(([stream, p]) => shown(stream, p.line, p.cut));
      const body = [...lines, ...partial];
      const showing = last === undefined ? body : last > 0 ? body.slice(-last) : [];
      const notShown = dropped + body.length - showing.length;
      const head = `${label}: ${ending ?? "still running when the test ended"}`;
      const cut = notShown > 0 ? [`(${notShown} earlier lines not shown)`] : [];
      return [head, ...cut, ...showing].join("\n");
    },
  };
}

module.exports = { MAX_LINES, MAX_LINE_CHARS, outputRecorder };
