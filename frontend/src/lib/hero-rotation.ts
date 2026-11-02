/**
 * The home hero's auto-rotation: its rules and the interval that
 * applies them. The component owns only the reactive index and the
 * conditions it reads.
 *
 * Why extract: the same `if (rotationLength <= 1) return; if (paused)
 * return; if (reducedMotion) return;` chain was hard to verify at a
 * glance, and adding a fourth condition (e.g. "pause when the tab
 * isn't visible") would be invisible to tests inside the component.
 */

/** Index of the next rotation slot. Wraps modulo `total`. Returns 0
 *  for an empty rotation so the caller can hand it back to a state
 *  setter without branching. */
export function nextHeroIndex(current: number, total: number): number {
	if (total <= 0) return 0;
	return (current + 1) % total;
}

/** Whether the rotation interval should be running given current
 *  conditions. Three guards, all of equal weight: a single-item
 *  rotation has nothing to rotate to; a paused rotation is paused
 *  on purpose; reduced-motion users opted out of every kind of
 *  ambient motion. */
export function shouldRunHeroRotation(opts: {
	rotationLength: number;
	paused: boolean;
	prefersReducedMotion: boolean;
}): boolean {
	if (opts.rotationLength <= 1) return false;
	if (opts.paused) return false;
	if (opts.prefersReducedMotion) return false;
	return true;
}

/** How long each title holds the hero before the next. */
export const HERO_ROTATE_MS = 6000;

/** A repeating timer; `browserIntervalHost()` is the window's. */
export interface IntervalHost {
	every(cb: () => void, ms: number): number;
	cancel(id: number): void;
}

/** The window's interval, wrapped in arrows: called as a method of
 *  this object, `setInterval` throws "Illegal invocation" in Chromium. */
export function browserIntervalHost(): IntervalHost {
	return {
		every: (cb, ms) => window.setInterval(cb, ms),
		cancel: (id) => window.clearInterval(id)
	};
}

/**
 * Runs the rotation while `conditions` allow it: calls `advance` every
 * `HERO_ROTATE_MS`. Returns the stop, which the page's effect hands
 * back so a change of conditions (a pause, the motion preference)
 * stops the interval and the next run decides again.
 */
export function startHeroRotation(
	conditions: Parameters<typeof shouldRunHeroRotation>[0],
	advance: () => void,
	host: IntervalHost = browserIntervalHost()
): () => void {
	if (!shouldRunHeroRotation(conditions)) return () => {};
	const id = host.every(advance, HERO_ROTATE_MS);
	return () => host.cancel(id);
}
