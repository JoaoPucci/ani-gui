// The busy notice's countdown: the seconds left until a held write goes
// again, read from one deadline rather than counted down, so a late or
// skipped tick shows the true remainder and nothing drifts. Pure, with
// the clock and the timer injected.

/** The whole seconds left until `deadlineMs` at `nowMs`: never more than
 *  the time truly remaining, and never less than zero. */
export function secondsLeft(deadlineMs: number, nowMs: number): number {
	return Math.max(0, Math.floor((deadlineMs - nowMs) / 1000));
}

export interface CountdownDeps {
	/** The clock, in milliseconds. */
	now: () => number;
	/** Call `tick` every `ms`; returns what stops it. */
	every: (ms: number, tick: () => void) => () => void;
	/** Show `seconds` left — zero once the deadline has passed. */
	show: (seconds: number) => void;
}

/**
 * Show the seconds left until `deadlineMs` now and then once a second,
 * each time read afresh from the deadline, until it reaches zero — shown
 * once, then the countdown stops itself — or the returned stop is
 * called.
 */
export function startCountdown(deadlineMs: number, deps: CountdownDeps): () => void {
	const first = secondsLeft(deadlineMs, deps.now());
	deps.show(first);
	if (first === 0) return () => {};
	let cancel: (() => void) | null = null;
	const stop = () => {
		cancel?.();
		cancel = null;
	};
	cancel = deps.every(1000, () => {
		const seconds = secondsLeft(deadlineMs, deps.now());
		deps.show(seconds);
		if (seconds === 0) stop();
	});
	return stop;
}
