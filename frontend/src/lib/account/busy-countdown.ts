// The busy notice's countdown: the seconds left until a held write goes
// again, read from one deadline rather than counted down, so a late or
// skipped tick shows the true remainder and nothing drifts. The number
// is the whole second the remainder is in — rounded up, so the notice
// never says the save is going before it does — and each tick is timed
// for the moment that second is over. Pure, with the clock and the
// timer injected.

/** The whole seconds left until `deadlineMs` at `nowMs`, rounded up:
 *  the second the remainder is in, and zero only once nothing is left. */
export function secondsLeft(deadlineMs: number, nowMs: number): number {
	return Math.max(0, Math.ceil((deadlineMs - nowMs) / 1000));
}

/** How long until the number `secondsLeft` shows next changes. */
function untilNextSecond(deadlineMs: number, nowMs: number): number {
	const remaining = deadlineMs - nowMs;
	return remaining - (Math.ceil(remaining / 1000) - 1) * 1000;
}

export interface CountdownDeps {
	/** The clock, in milliseconds. */
	now: () => number;
	/** Call `tick` once, after `ms`; returns what cancels it. */
	after: (ms: number, tick: () => void) => () => void;
	/** Show `seconds` left — zero once the deadline has passed. */
	show: (seconds: number) => void;
}

/**
 * Show the seconds left until `deadlineMs` now, and again each time the
 * second shown is over, each time read afresh from the deadline, until
 * it reaches zero — shown once, at the deadline, then the countdown
 * stops itself — or the returned stop is called.
 */
export function startCountdown(deadlineMs: number, deps: CountdownDeps): () => void {
	let cancel: (() => void) | null = null;
	const tick = () => {
		const now = deps.now();
		const seconds = secondsLeft(deadlineMs, now);
		deps.show(seconds);
		cancel = seconds === 0 ? null : deps.after(untilNextSecond(deadlineMs, now), tick);
	};
	tick();
	return () => {
		cancel?.();
		cancel = null;
	};
}
