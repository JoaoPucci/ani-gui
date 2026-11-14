'use strict';

// Stopping the app an e2e spec launched: its process group, and nothing
// that merely sits near it in the process table.
//
// Playwright starts the app detached (its process launcher spawns with
// `detached: true` off Windows), so the app's main process leads a
// process group, and a session, of its own, whose id is its pid.
// Chromium's helper processes and the app's keyring helper run in that
// group. The backend does not: the app starts it in a group of its own,
// and the backend shuts itself down once the app's end of its stdin
// closes, however the app ended (lib/backend-spawn.cjs).
//
// A group id is only worth signalling while it is provably the app's:
//
//   - While Node has not reaped the main process. `exitCode` and
//     `signalCode` stay null until Node reports the exit, and an
//     unreaped pid — the group's id with it — is not handed to anything
//     else. Before every signal the main process is also checked in
//     /proc: it still leads its group, and runs the binary the test
//     launched. libuv reaps every exited child before it reports any of
//     them, so for a moment a reaped child can still read as unreaped;
//     its /proc/<pid> entry is gone by then, and the check refuses to
//     signal.
//   - As its exit is reported. The 'exit' handler runs right after the
//     reap, before anything else in the test runs, and a group id is
//     not given out while anything is left in the group, so a signal
//     sent there reaches only what the app left behind.
//     lib/backend-spawn.cjs's reapOnExit stops the backend's group on
//     the same reasoning.
//   - Never after. Once the group empties its id is free, and a later
//     signal could reach whoever is handed it.
//
// Linux only (reads /proc), as the e2e suite is.

const fs = require('node:fs');

/**
 * The fields of a /proc/<pid>/stat line a stop reads: the state letter,
 * the process group, the session. The command name in parentheses may
 * itself hold spaces and parentheses, so fields are counted from the
 * last closing one. Null for a line that does not parse.
 */
function statFields(text) {
	const close = String(text).lastIndexOf(')');
	if (close < 0) return null;
	// state, parent, process group, session, ...
	const f = String(text)
		.slice(close + 2)
		.split(' ');
	const pgrp = Number(f[2]);
	const session = Number(f[3]);
	if (!/^[A-Za-z]$/.test(f[0] || '') || !Number.isInteger(pgrp) || !Number.isInteger(session)) {
		return null;
	}
	return { state: f[0], pgrp, session };
}

function exited(child) {
	return child.exitCode !== null || child.signalCode !== null;
}

/**
 * Whether `child` is still the group leader the test launched: unreaped,
 * leading its own group, running `exe` (when given). 'exited' once Node
 * has reaped it; 'zombie' when it has died and awaits that reap, or is
 * exiting (its executable already released). Throws
 * when the process is not one this module may signal.
 */
function leaderState(child, { exe, readFile, readLink }) {
	if (exited(child)) return 'exited';
	const pid = child.pid;
	let stat;
	try {
		stat = statFields(readFile(`/proc/${pid}/stat`, 'utf8'));
	} catch {
		stat = null;
	}
	// Unreaped, yet unreadable: nothing here can vouch for it.
	if (!stat) throw new Error(`process ${pid} cannot be read in /proc; not signalled`);
	if (stat.pgrp !== pid) {
		throw new Error(`process ${pid} does not lead a process group of its own; not signalled`);
	}
	if (stat.state === 'Z') return 'zombie';
	if (exe !== undefined) {
		let running;
		try {
			running = readLink(`/proc/${pid}/exe`);
		} catch (err) {
			// Unreaped, leading its group, and its executable already
			// released: it is exiting, and its reap will sweep the group.
			if (err && err.code === 'ENOENT') return 'zombie';
			throw new Error(`process ${pid}'s binary cannot be read (${err && err.code}); not signalled`);
		}
		// The binary rebuilt under it since the launch: still the process
		// the test started, from the same path.
		running = running.replace(/ \(deleted\)$/, '');
		if (running !== exe) {
			throw new Error(`process ${pid} runs ${running}, not ${exe}; not signalled`);
		}
	}
	return 'running';
}

function within(promise, ms) {
	let timer;
	return Promise.race([
		promise.then(() => true),
		new Promise((resolve) => {
			timer = setTimeout(() => resolve(false), ms);
		}),
	]).finally(() => clearTimeout(timer));
}

/**
 * Stop the process group `child` leads — the app a spec launched — and
 * resolve with how it ended:
 *
 *   - 'exited': it had already exited and been reaped. Nothing is
 *     signalled; its group id can no longer be shown to be the app's.
 *   - 'stopped': it exited after SIGTERM to the group.
 *   - 'killed': it outlived SIGTERM by `termMs`, and exited after
 *     SIGKILL to the same group — or, found exiting by then, without
 *     one.
 *
 * Whatever the app leaves in its group when it exits is killed in the
 * turn its exit is reported. Throws, signalling nothing further, when
 * the process is not the leader this module may signal (see
 * `leaderState`), or when it outlives SIGKILL by `killMs`.
 *
 * `kill`, `readFile` and `readLink` are process.kill, fs.readFileSync
 * and fs.readlinkSync; tests replace them.
 */
async function stopOwnGroup(
	child,
	{
		exe,
		termMs = 5000,
		killMs = 5000,
		kill = (pid, signal) => process.kill(pid, signal),
		readFile = fs.readFileSync,
		readLink = fs.readlinkSync,
	} = {},
) {
	const io = { exe, readFile, readLink };
	const first = leaderState(child, io);
	if (first === 'exited') return 'exited';
	const pid = child.pid;
	const signal = (sig) => {
		try {
			kill(-pid, sig);
		} catch (err) {
			if (!err || err.code !== 'ESRCH') throw err;
		}
	};
	const gone = new Promise((resolve) => {
		child.once('exit', () => {
			// The reap's own turn: the group id is still the app's. An
			// event handler has nowhere to report a failure to; the
			// group is the app's either way.
			try {
				signal('SIGKILL');
			} catch {
				// Not ESRCH, and nothing to do about it here.
			}
			resolve();
		});
	});
	if (first === 'running') signal('SIGTERM');
	if (await within(gone, termMs)) return 'stopped';
	const second = leaderState(child, io);
	if (second === 'exited') return 'stopped';
	if (second === 'running') signal('SIGKILL');
	if (await within(gone, killMs)) return 'killed';
	throw new Error(`process group ${pid} outlived SIGKILL by ${killMs} ms`);
}

/**
 * Whether nothing is left in the process group `pgid`: signal 0, which
 * delivers nothing, finds no member. Once a group has emptied, its id
 * can be handed to someone else's new group, which then reads as not
 * empty — a false failure, never a signal sent.
 */
function groupEmpty(pgid, { kill = (pid, signal) => process.kill(pid, signal) } = {}) {
	try {
		kill(-pgid, 0);
		return false;
	} catch (err) {
		if (err && err.code === 'ESRCH') return true;
		throw err;
	}
}

module.exports = { groupEmpty, statFields, stopOwnGroup };
