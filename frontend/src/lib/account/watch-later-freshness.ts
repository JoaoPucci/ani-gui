/**
 * The Watch Later rail's freshness line, over time.
 *
 * `./watch-later-status` says what the line reads for a given status
 * and clock. This module is what moves those inputs: when the line
 * reads the trackers' last-pulled times again (`stamped`), what the
 * clock reads for it (`now`), the label held for its width while a
 * refresh runs (`held`), and what a refresh says to assistive
 * technology (`announce`). The home page keeps a copy of the state in
 * a Svelte `$state` and hands this module the events it observes.
 *
 *   - The clock moves only when the age on screen next changes
 *     (`nextAgeChange`), by one timer re-armed each time it fires, so
 *     the page re-renders once per change instead of on a fixed tick.
 *     A status that shows no age (refreshing, never, failed, nothing
 *     to report) holds no timer.
 *   - A refresh starting holds the line's current label and announces
 *     itself; settling moves the clock, has the times read again,
 *     releases the held label, and announces the line as it then
 *     reads. Nothing else is announced: the line ticks every minute
 *     and would chatter as a live region.
 *   - A tracker write clears a stamp whatever became of it, and the
 *     rail being read again may follow a pull that recorded one; both
 *     have the times read again.
 */

import { nextAgeChange, type WatchLaterStatus } from './watch-later-status';

export interface FreshnessState {
	/** The clock the line's age is read against. */
	now: number;
	/** Moves whenever the last-pulled times may have changed. */
	stamped: number;
	/** The label held (hidden) for its width while a refresh runs. */
	held: string | null;
	/** The live region's text. */
	announce: string;
}

export interface FreshnessClock {
	now(): number;
	schedule(fn: () => void, ms: number): unknown;
	cancel(handle: unknown): void;
}

export interface Freshness {
	readonly state: FreshnessState;
	/** The status the line now shows; re-arms or drops the age timer. */
	track(status: WatchLaterStatus | null): void;
	/** A refresh started while the line read `label`. */
	refreshStarted(label: string | null, refreshingText: string): void;
	/** A refresh settled. `label` reads the line once the change has landed. */
	refreshSettled(label: () => string | null): void;
	/** The last-pulled times may have changed: a tracker write cleared
	 *  one, or the rail was read again after a pull. */
	stampsChanged(): void;
	/** The page is going away: no timer outlives it, and nothing changes after. */
	dispose(): void;
}

export function createFreshness(
	clock: FreshnessClock,
	onChange: (state: FreshnessState) => void
): Freshness {
	let state: FreshnessState = { now: clock.now(), stamped: 0, held: null, announce: '' };
	let timer: unknown = null;
	let tracked: WatchLaterStatus | null = null;
	let disposed = false;

	const set = (patch: Partial<FreshnessState>) => {
		if (disposed) return;
		state = { ...state, ...patch };
		onChange(state);
	};
	const stop = () => {
		if (timer !== null) clock.cancel(timer);
		timer = null;
	};
	const arm = () => {
		stop();
		if (disposed || tracked?.kind !== 'updated') return;
		const at = tracked.at;
		timer = clock.schedule(
			() => {
				timer = null;
				set({ now: clock.now() });
				arm();
			},
			nextAgeChange(at, state.now)
		);
	};

	return {
		get state() {
			return state;
		},
		track(status) {
			tracked = status;
			arm();
		},
		refreshStarted(label, refreshingText) {
			set({ held: label, announce: refreshingText });
		},
		refreshSettled(label) {
			set({ now: clock.now(), stamped: state.stamped + 1, held: null });
			set({ announce: label() ?? '' });
		},
		stampsChanged() {
			set({ stamped: state.stamped + 1 });
		},
		dispose() {
			stop();
			disposed = true;
		}
	};
}
