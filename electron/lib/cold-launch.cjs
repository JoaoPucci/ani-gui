// Retry harness for the e2e suites' Electron cold launch.
//
// The launch died, rarely, with Playwright's closed-target signature:
// the first window torn down underneath the about:blank bounce on a
// cold launch. The known cause was the app's: the bounce superseded a
// first page that had not finished loading, the boot treated the
// rejected loadURL as fatal and exited, and while the orphaned
// backend ran, this retry's close of the exited app never returned.
// lib/startup.cjs now keeps that window and stops the backend on a
// failed boot.
//
// The retry stays as a net for a death that carries no information
// about the code under test: it is the one failure a relaunch may
// absorb. Everything else — assertions, spawn errors — propagates
// untouched.

/** Whether an error is Playwright's closed-target signature. */
function isClosedTargetError(err) {
	const text = err && err.message ? err.message : String(err);
	return text.includes('Target page, context or browser has been closed');
}

/**
 * Run `attempt`; when it dies with the closed-target signature, run
 * `cleanup` (best-effort — a close racing the dead process is
 * expected, and so is one that never settles, which is abandoned
 * after `cleanupTimeoutMs`) and try again, up to `retries` extra
 * times. The last closed-target error surfaces once retries are
 * exhausted.
 */
async function withColdLaunchRetry(
	attempt,
	{ retries = 1, cleanup, cleanupTimeoutMs = 5000 } = {},
) {
	let lastErr;
	for (let i = 0; i <= retries; i += 1) {
		try {
			return await attempt(i);
		} catch (err) {
			if (!isClosedTargetError(err)) throw err;
			lastErr = err;
			if (cleanup) {
				let timer;
				try {
					await Promise.race([
						cleanup(err),
						new Promise((resolve) => {
							timer = setTimeout(resolve, cleanupTimeoutMs);
						}),
					]);
				} catch {
					// The dead app may already be gone; the retry is the point.
				} finally {
					clearTimeout(timer);
				}
			}
		}
	}
	throw lastErr;
}

/**
 * SIGKILL `pid` and every descendant, found by walking the parent
 * links before anything is killed. A group kill is not enough: the
 * backend runs in its own session, and while it lives it holds
 * descriptors inherited from the app, so Playwright's close() of a
 * dead app does not settle. Linux only (reads `ps`), as the e2e
 * suites are.
 */
function killTree(pid) {
	const { execFileSync } = require('node:child_process');
	const children = new Map();
	for (const line of execFileSync('ps', ['-eo', 'pid=,ppid='], { encoding: 'utf8' }).split('\n')) {
		const [child, parent] = line.trim().split(/\s+/).map(Number);
		if (!child) continue;
		if (!children.has(parent)) children.set(parent, []);
		children.get(parent).push(child);
	}
	const tree = [pid];
	for (let i = 0; i < tree.length; i += 1) tree.push(...(children.get(tree[i]) || []));
	for (const each of tree) {
		try {
			process.kill(each, 'SIGKILL');
		} catch {
			// Already gone.
		}
	}
}

module.exports = { isClosedTargetError, killTree, withColdLaunchRetry };
