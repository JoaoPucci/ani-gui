'use strict';

// The main process's stdout and stderr, kept from taking the app down.
//
// They carry diagnostics only: the main process's own lines, the
// renderer console it relays and the backend's log ([backend] lines,
// lib/log-redact.cjs). Nothing the app does depends on them being read,
// and they can stop being writable at any moment the app does not
// control:
//
//   - the terminal the app was started from closes: every write to it
//     fails with EIO from then on;
//   - the reader of a pipe goes away (`ani-gui | head`, a launcher that
//     exits): EPIPE, since Node ignores SIGPIPE;
//   - on Windows, a pipe the output goes to breaks, which libuv reports
//     as EPIPE, EOF or ECONNRESET.
//
// Node reports such a failure as an 'error' event on the stream. With
// no listener that is an uncaught exception, which in Electron's main
// process opens the "A JavaScript error occurred" dialog and leaves the
// app waiting on it: the app the user closed never quits. console.*
// covers only its own write; a direct `process.stdout.write`, which the
// backend log relay makes, has nothing at all. And a stream whose file
// is written synchronously (stdout redirected to a file) throws out of
// write() itself.
//
// So every error on these streams is taken as the stream being gone,
// whatever its code: a failed write of a diagnostic line is not worth a
// dialog, and a code this list does not name (Windows reports broken
// handles several ways) would otherwise bring the dialog back. Chromium
// treats its own log output the same way and ignores a failed write to
// stderr. Once one write has failed, the stream takes no more: later
// writes are dropped, their callbacks still called, so no caller waits
// on a write that will never happen and no write retries a dead
// terminal on every line.
//
// What this cannot do: on Linux, Node puts the terminal back as it
// found it when the process exits (node::ResetStdio), and on a terminal
// that has hung up that step aborts the process (SIGABRT) — after the
// app has shut down, but as a crash. That happens in native code at
// exit. Node skips a descriptor that has been closed or now names
// another file, but JavaScript has no dup2 to swap the terminal out
// atomically, and closing descriptors 0 to 2 would let any thread's
// next open take them (Chromium's own log on fd 2 included). Nothing
// the main process can safely do avoids it; only how the app is
// launched (its stdio not being the terminal) does. The packaged Linux
// launcher does that for a launch from a terminal
// (lib/linux-launcher.cjs): the app gets /dev/null and pipes, and its
// output reaches the terminal through the launcher, so a terminal that
// closes leaves the app the EPIPE above, never the abort.

/** A write that goes nowhere and reports itself done. */
function discard(_chunk, encoding, callback) {
	const done = typeof encoding === 'function' ? encoding : callback;
	if (typeof done === 'function') process.nextTick(done);
	return true;
}

/** Keeps `stream`'s write failures from reaching anyone: an 'error'
 *  event or a throwing write marks it gone, and from then on every
 *  write is dropped. */
function guardStream(stream) {
	const write = stream.write;
	let gone = false;
	const lose = () => {
		if (gone) return;
		gone = true;
		stream.write = discard;
	};
	// Kept for good: a write already under way when the stream went can
	// still report its failure after the stream is marked gone.
	stream.on('error', lose);
	stream.write = function guardedWrite(chunk, encoding, callback) {
		try {
			return write.call(this, chunk, encoding, callback);
		} catch {
			lose();
			return discard(chunk, encoding, callback);
		}
	};
}

/** Guards `proc`'s stdout and stderr (see the top of this file). Called
 *  first thing in the main process, before anything writes to either. */
function guardStdio(proc, { guard = guardStream } = {}) {
	guard(proc.stdout);
	guard(proc.stderr);
}

module.exports = { guardStdio, guardStream };
