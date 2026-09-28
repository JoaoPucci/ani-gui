/**
 * The timer behind a held retry. A hold's delay is capped to the
 * buffer at the moment of the failure, but the buffer keeps draining
 * while the timer waits, and not always at the rate the cap assumed:
 * a rate change or a seek after the hold was armed drains it sooner,
 * and the media would run out before the engine was asked again, with
 * nothing shown. So the timer watches the runway while it waits, and
 * fires once the margin is about to go or the delay is up, whichever
 * comes first.
 */

/** How often a pending retry looks at the runway. */
export const HELD_RETRY_TICK_MS = 500;

export interface HeldRetry {
	/** The hold's delay, from now. */
	delayMs: number;
	/** Seconds of playback the buffer is good for, read at each tick. */
	runwaySeconds: () => number;
	/** The retry fires once the runway is down to this. */
	marginSeconds: number;
	fire: () => void;
}

/** Arms the retry; the returned function cancels it. */
export function scheduleHeldRetry(retry: HeldRetry): () => void {
	const due = Date.now() + retry.delayMs;
	let timer: ReturnType<typeof setTimeout> | null = null;
	const tick = () => {
		timer = null;
		const remaining = due - Date.now();
		if (remaining <= 0 || retry.runwaySeconds() <= retry.marginSeconds) {
			retry.fire();
			return;
		}
		timer = setTimeout(() => tick(), Math.min(remaining, HELD_RETRY_TICK_MS));
	};
	timer = setTimeout(() => tick(), Math.min(retry.delayMs, HELD_RETRY_TICK_MS));
	return () => {
		if (timer !== null) clearTimeout(timer);
		timer = null;
	};
}
