// The e2e suites' Electron cold launch dies, rarely, with
// Playwright's closed-target signature: the first window is torn
// down underneath the about:blank bounce while the app is still
// settling, and the spec fails at page.goto before asserting
// anything. The retry harness distinguishes exactly that signature
// from real failures — an assertion or launch error must propagate
// immediately, or the retry hides bugs instead of absorbing flakes.

const test = require('node:test');
const assert = require('node:assert/strict');

const { isClosedTargetError, withColdLaunchRetry } = require('./cold-launch.cjs');

const closedTarget = () =>
	new Error('page.goto: Target page, context or browser has been closed');

test('the Playwright closed-target signature is recognized', () => {
	assert.equal(isClosedTargetError(closedTarget()), true);
	assert.equal(isClosedTargetError(new Error('expect(received).toBe(expected)')), false);
	assert.equal(isClosedTargetError('Target page, context or browser has been closed'), true);
});

test('a clean launch runs once and returns its handle', async () => {
	let attempts = 0;
	const got = await withColdLaunchRetry(async () => {
		attempts += 1;
		return 'handle';
	});
	assert.equal(got, 'handle');
	assert.equal(attempts, 1);
});

test('a closed-target death is cleaned up and relaunched', async () => {
	let attempts = 0;
	let cleaned = 0;
	const got = await withColdLaunchRetry(
		async () => {
			attempts += 1;
			if (attempts === 1) throw closedTarget();
			return 'second';
		},
		{
			cleanup: async () => {
				cleaned += 1;
			},
		},
	);
	assert.equal(got, 'second');
	assert.equal(attempts, 2);
	assert.equal(cleaned, 1);
});

test('any other failure propagates without a retry', async () => {
	let attempts = 0;
	await assert.rejects(
		withColdLaunchRetry(async () => {
			attempts += 1;
			throw new Error('spawn ENOENT');
		}),
		/spawn ENOENT/,
	);
	assert.equal(attempts, 1);
});

test('exhausted retries surface the last closed-target error', async () => {
	let attempts = 0;
	await assert.rejects(
		withColdLaunchRetry(
			async () => {
				attempts += 1;
				throw closedTarget();
			},
			{ retries: 2 },
		),
		/has been closed/,
	);
	assert.equal(attempts, 3);
});

test('a cleanup that itself fails does not mask the retry', async () => {
	let attempts = 0;
	const got = await withColdLaunchRetry(
		async () => {
			attempts += 1;
			if (attempts === 1) throw closedTarget();
			return 'ok';
		},
		{
			cleanup: async () => {
				throw new Error('close raced the dead process');
			},
		},
	);
	assert.equal(got, 'ok');
});

// CI run 37241086339: the dead app's close() never settled, so the
// cleanup ate the whole test timeout and the relaunch never ran. A
// cleanup gets a bound; past it the harness relaunches regardless.
test('a cleanup that never settles is abandoned and the relaunch runs', { timeout: 2000 }, async () => {
	let attempts = 0;
	const got = await withColdLaunchRetry(
		async () => {
			attempts += 1;
			if (attempts === 1) throw closedTarget();
			return 'relaunched';
		},
		{
			cleanupTimeoutMs: 50,
			cleanup: () => new Promise(() => {}),
		},
	);
	assert.equal(got, 'relaunched');
	assert.equal(attempts, 2);
});

// The dead app's close() hangs because the backend outlives it: the
// backend runs in its own session, out of reach of a group kill, and
// holds descriptors it inherited from the app. Killing the app's
// whole tree, its own-session descendants included, lets close settle.
test('killTree takes a process and a descendant in its own session', { skip: process.platform !== 'linux' }, async () => {
	const { spawn } = require('node:child_process');
	const { killTree } = require('./cold-launch.cjs');
	const parent = spawn('sh', ['-c', 'setsid sleep 300 & echo $!; wait'], {
		stdio: ['ignore', 'pipe', 'ignore'],
	});
	const child = Number(
		await new Promise((resolve) => parent.stdout.once('data', (d) => resolve(String(d).trim()))),
	);
	const alive = (pid) => {
		try {
			process.kill(pid, 0);
			return true;
		} catch {
			return false;
		}
	};
	try {
		assert.equal(alive(child), true);
		killTree(parent.pid);
		await new Promise((resolve) => setTimeout(resolve, 200));
		assert.equal(alive(parent.pid) && parent.exitCode === null && parent.signalCode === null, false);
		assert.equal(alive(child), false, 'the own-session descendant is gone');
	} finally {
		for (const pid of [child, parent.pid]) {
			try {
				process.kill(pid, 'SIGKILL');
			} catch {}
		}
	}
});
