// Retry harness for the e2e suites' Electron cold launch.
//
// The launch dies, rarely, with Playwright's closed-target signature:
// the first window is torn down underneath the about:blank bounce
// while a cold app is still settling. That death carries no
// information about the code under test, so it is the one failure a
// relaunch may absorb. Everything else — assertions, spawn errors —
// propagates untouched.

/** Whether an error is Playwright's closed-target signature. */
function isClosedTargetError(err) {
	const text = err && err.message ? err.message : String(err);
	return text.includes('Target page, context or browser has been closed');
}

/**
 * Run `attempt`; when it dies with the closed-target signature, run
 * `cleanup` (best-effort — a close racing the dead process is
 * expected) and try again, up to `retries` extra times. The last
 * closed-target error surfaces once retries are exhausted.
 */
async function withColdLaunchRetry(attempt, { retries = 1, cleanup } = {}) {
	let lastErr;
	for (let i = 0; i <= retries; i += 1) {
		try {
			return await attempt(i);
		} catch (err) {
			if (!isClosedTargetError(err)) throw err;
			lastErr = err;
			if (cleanup) {
				try {
					await cleanup(err);
				} catch {
					// The dead app may already be gone; the retry is the point.
				}
			}
		}
	}
	throw lastErr;
}

module.exports = { isClosedTargetError, withColdLaunchRetry };
