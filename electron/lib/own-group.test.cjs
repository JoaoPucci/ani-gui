// How an e2e spec stops the app it launched: the process group the app
// leads, signalled only while that group is provably the app's, and
// nothing outside it. The processes here are real (sh and sleep), each
// started detached as Playwright starts the app, so each leads a group
// of its own.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { spawn } = require('node:child_process');
const fc = require('fast-check');

const { groupEmpty, statFields, stopOwnGroup } = require('./own-group.cjs');

const linuxOnly = { skip: process.platform !== 'linux' };
const SH = '/bin/sh';
const shExe = () => fs.realpathSync(SH);

/** Run `script` under sh, leading a group of its own; resolves once it
 *  has printed its first line (a pid it started, or "ready"). */
async function leader(script, { detached = true } = {}) {
	const child = spawn(SH, ['-c', script], { stdio: ['ignore', 'pipe', 'ignore'], detached });
	const line = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.stdout.once('data', (d) => resolve(String(d).trim()));
	});
	// Nothing more is read; a member still holding the pipe must not
	// keep the test process waiting on it.
	child.stdout.destroy();
	return { child, line };
}

// A killed process whose parent is gone waits as a zombie until PID 1
// reaps it, and some container runtimes' PID 1 never does. A zombie runs
// nothing and holds no descriptors, so it counts as gone here.
function alive(pid) {
	try {
		process.kill(pid, 0);
	} catch {
		return false;
	}
	try {
		return statFields(fs.readFileSync(`/proc/${pid}/stat`, 'utf8')).state !== 'Z';
	} catch {
		return false;
	}
}

async function until(predicate, ms = 2000) {
	const deadline = Date.now() + ms;
	while (!predicate()) {
		if (Date.now() > deadline) return false;
		await new Promise((resolve) => setTimeout(resolve, 20));
	}
	return true;
}

/** Kill what a test left running: the leader's group while the leader
 *  is unreaped (the group is then provably its own), and any process
 *  outside it by the pid the leader reported, while that process is
 *  still running (a running process keeps its pid). */
function cleanup(child, ...outside) {
	if (!exitedAlready(child)) {
		try {
			process.kill(-child.pid, 'SIGKILL');
		} catch {
			// Not a group leader, or nothing left in its group.
		}
		child.kill('SIGKILL');
	}
	for (const pid of outside.filter(alive)) {
		try {
			process.kill(pid, 'SIGKILL');
		} catch {
			// Already gone.
		}
	}
}

/** A kill that records instead of signalling. */
function spyKill() {
	const calls = [];
	const kill = (pid, signal) => {
		calls.push([pid, signal]);
	};
	return { calls, kill };
}

test('a stat line parses whatever the command name holds', () => {
	const comm = fc.string({ maxLength: 16 }).filter((s) => !s.includes('\n'));
	const id = fc.integer({ min: 1, max: 4194304 });
	fc.assert(
		fc.property(id, comm, fc.constantFrom('R', 'S', 'D', 'Z', 'T'), id, id, id, (pid, name, state, parent, pgrp, session) => {
			const line = `${pid} (${name}) ${state} ${parent} ${pgrp} ${session} 34816 0 -1 4194560\n`;
			assert.deepEqual(statFields(line), { state, pgrp, session });
		}),
	);
});

test('a line that is not a stat line parses to nothing', () => {
	for (const text of ['', 'garbage', '12 (sh)', '12 (sh) S x y z']) {
		assert.equal(statFields(text), null, JSON.stringify(text));
	}
});

test('the group is stopped with SIGTERM, the leader and what it started in it', linuxOnly, async () => {
	const { child, line } = await leader('sleep 300 & echo $!; wait');
	const member = Number(line);
	try {
		assert.equal(await stopOwnGroup(child, { exe: shExe() }), 'stopped');
		assert.equal(await until(() => !alive(member)), true, 'the member is gone');
	} finally {
		cleanup(child);
	}
});

test('a group that ignores SIGTERM is killed', linuxOnly, async () => {
	const { child, line } = await leader("trap '' TERM; sleep 300 & echo $!; wait");
	const member = Number(line);
	try {
		assert.equal(await stopOwnGroup(child, { exe: shExe(), termMs: 300 }), 'killed');
		assert.equal(await until(() => !alive(member)), true, 'the member is gone');
	} finally {
		cleanup(child);
	}
});

test('what the leader leaves in its group when it exits is killed as it exits', linuxOnly, async () => {
	// The leader quits on SIGTERM; the member ignores it and would
	// outlive the leader.
	const { child, line } = await leader(
		`trap 'exit 0' TERM; (trap '' TERM; exec sleep 300) & echo $!; wait`,
	);
	const member = Number(line);
	try {
		// It ignores SIGTERM once it runs sleep, which it execs after the trap.
		const sleeping = () => {
			try {
				return fs.realpathSync(`/proc/${member}/exe`).endsWith('sleep');
			} catch {
				return false;
			}
		};
		assert.equal(await until(sleeping), true);
		assert.equal(await stopOwnGroup(child, { exe: shExe() }), 'stopped');
		assert.equal(await until(() => !alive(member)), true, 'the member is gone');
	} finally {
		cleanup(child);
	}
});

test('a process the leader started outside its group is left alone', linuxOnly, async () => {
	// As the backend is: in a session of its own, which the app does not
	// signal and this does not either.
	const { child, line } = await leader('setsid sleep 300 & echo $!; wait');
	const outsider = Number(line);
	try {
		// $! is printed before the background process has called setsid.
		const ownSession = () => {
			try {
				return statFields(fs.readFileSync(`/proc/${outsider}/stat`, 'utf8')).session === outsider;
			} catch {
				return false;
			}
		};
		assert.equal(await until(ownSession), true, 'the outsider is in a session of its own');
		assert.equal(await stopOwnGroup(child, { exe: shExe() }), 'stopped');
		await new Promise((resolve) => setTimeout(resolve, 200));
		assert.equal(alive(outsider), true, 'the process outside the group was not signalled');
	} finally {
		cleanup(child, outsider);
	}
});

test('a process that does not lead its own group is not signalled', linuxOnly, async () => {
	const { child } = await leader('echo ready; exec sleep 300', { detached: false });
	const spy = spyKill();
	try {
		await assert.rejects(stopOwnGroup(child, { exe: shExe(), kill: spy.kill }), /does not lead a process group/);
		assert.deepEqual(spy.calls, []);
		assert.equal(alive(child.pid), true);
	} finally {
		cleanup(child);
	}
});

test('a leader running another binary than the one launched is not signalled', linuxOnly, async () => {
	const { child } = await leader('echo ready; exec sleep 300');
	const spy = spyKill();
	try {
		await assert.rejects(stopOwnGroup(child, { exe: shExe(), kill: spy.kill }), /not signalled/);
		assert.deepEqual(spy.calls, []);
		assert.equal(alive(child.pid), true);
	} finally {
		cleanup(child);
	}
});

test('a leader already reaped is not signalled: its group id may be anyone’s by now', linuxOnly, async () => {
	const { child } = await leader('echo ready');
	await new Promise((resolve) => (exitedAlready(child) ? resolve() : child.once('exit', resolve)));
	const spy = spyKill();
	assert.equal(await stopOwnGroup(child, { exe: shExe(), kill: spy.kill }), 'exited');
	assert.deepEqual(spy.calls, []);
});

function exitedAlready(child) {
	return child.exitCode !== null || child.signalCode !== null;
}

test('a group that outlives SIGKILL fails the stop', async () => {
	// A stand-in that never reports an exit, as a process stuck in the
	// kernel would not; every check in /proc passes.
	const { EventEmitter } = require('node:events');
	const child = Object.assign(new EventEmitter(), { pid: 4242, exitCode: null, signalCode: null });
	const spy = spyKill();
	await assert.rejects(
		stopOwnGroup(child, {
			exe: '/opt/app/bin',
			termMs: 10,
			killMs: 10,
			kill: spy.kill,
			readFile: () => '4242 (app) S 1 4242 4242 0 -1',
			readLink: () => '/opt/app/bin',
		}),
		/outlived SIGKILL/,
	);
	assert.deepEqual(spy.calls, [
		[-4242, 'SIGTERM'],
		[-4242, 'SIGKILL'],
	]);
});

test('a leader that has died and awaits its reap gets no SIGTERM; its group is swept as it is reaped', async () => {
	const { EventEmitter } = require('node:events');
	const child = Object.assign(new EventEmitter(), { pid: 4243, exitCode: null, signalCode: null });
	const spy = spyKill();
	const stop = stopOwnGroup(child, {
		exe: '/opt/app/bin',
		kill: spy.kill,
		readFile: () => '4243 (app) Z 1 4243 4243 0 -1',
		readLink: () => {
			throw new Error('a zombie has no exe');
		},
	});
	child.exitCode = 0;
	child.emit('exit', 0, null);
	assert.equal(await stop, 'stopped');
	assert.deepEqual(spy.calls, [[-4243, 'SIGKILL']]);
});

test('an empty group reads as empty, a group with a member does not', linuxOnly, async () => {
	const { child, line } = await leader('sleep 300 & echo $!; wait');
	const member = Number(line);
	try {
		assert.equal(groupEmpty(child.pid), false);
		await stopOwnGroup(child, { exe: shExe() });
		assert.equal(await until(() => groupEmpty(child.pid)), true);
	} finally {
		cleanup(child);
	}
});
