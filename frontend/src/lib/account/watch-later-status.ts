/**
 * What the Watch Later rail says about how fresh it is.
 *
 * The rail renders from a snapshot of each connected tracker's list,
 * and each tracker's snapshot carries its own last-pulled time
 * (`./watch-later-refresh`). The header line reports one of:
 *
 *   - `refreshing` — a pull is under way;
 *   - `never`      — some connected tracker has no recorded pull: it
 *                    was never pulled on this machine, or a local
 *                    sync invalidated it, so the rail cannot vouch for
 *                    that part of the list;
 *   - `updated`    — every tracker has a time; the rail is as fresh
 *                    as the OLDEST of them, which is the one it shows.
 *
 * The age is bucketed the way a person reads it — just now, minutes,
 * hours, days — and `nextAgeChange` says when the bucket rolls over,
 * so the page re-renders once per change instead of on a fixed tick.
 */

export type WatchLaterStatus =
	| { kind: 'refreshing' }
	| { kind: 'never' }
	| { kind: 'updated'; at: number };

/** The status for the connected trackers' last-pulled times, or
 *  `null` when no tracker is connected (there is nothing to report). */
export function watchLaterStatus(
	stamps: readonly (number | null)[],
	refreshing: boolean
): WatchLaterStatus | null {
	void stamps;
	void refreshing;
	throw new Error('not implemented');
}

export type Age = { unit: 'now' } | { unit: 'minute' | 'hour' | 'day'; count: number };

/** How long ago `at` was at `now`, in the bucket a person reads. */
export function ageOf(at: number, now: number): Age {
	void at;
	void now;
	throw new Error('not implemented');
}

/** Milliseconds from `now` until `ageOf(at, ·)` next changes. */
export function nextAgeChange(at: number, now: number): number {
	void at;
	void now;
	throw new Error('not implemented');
}

/** The localized words for each status, supplied by the caller so the
 *  module stays free of the message catalogue. */
export interface StatusCopy {
	refreshing(): string;
	never(): string;
	justNow(): string;
	updated(when: string): string;
}

/** The text the rail's header shows for `status` at `now`. */
export function statusLabel(
	status: WatchLaterStatus | null,
	now: number,
	locale: string,
	copy: StatusCopy
): string | null {
	void status;
	void now;
	void locale;
	void copy;
	throw new Error('not implemented');
}
