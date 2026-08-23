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
