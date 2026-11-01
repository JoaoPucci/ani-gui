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

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The status for the connected trackers' last-pulled times, or
 *  `null` when no tracker is connected (there is nothing to report). */
export function watchLaterStatus(
	stamps: readonly (number | null)[],
	refreshing: boolean
): WatchLaterStatus | null {
	if (stamps.length === 0) return null;
	if (refreshing) return { kind: 'refreshing' };
	let oldest = Infinity;
	for (const s of stamps) {
		if (s === null) return { kind: 'never' };
		oldest = Math.min(oldest, s);
	}
	return { kind: 'updated', at: oldest };
}

export type Age = { unit: 'now' } | { unit: 'minute' | 'hour' | 'day'; count: number };

/** How long ago `at` was at `now`, in the bucket a person reads. */
export function ageOf(at: number, now: number): Age {
	const elapsed = now - at;
	if (elapsed < MINUTE) return { unit: 'now' };
	if (elapsed < HOUR) return { unit: 'minute', count: Math.floor(elapsed / MINUTE) };
	if (elapsed < DAY) return { unit: 'hour', count: Math.floor(elapsed / HOUR) };
	return { unit: 'day', count: Math.floor(elapsed / DAY) };
}

/** Milliseconds from `now` until `ageOf(at, ·)` next changes. */
export function nextAgeChange(at: number, now: number): number {
	const elapsed = now - at;
	// A stamp ahead of the clock reads as just now until the clock has
	// caught up with it and then run a full minute past.
	if (elapsed < 0) return -elapsed + MINUTE;
	const step = elapsed < HOUR ? MINUTE : elapsed < DAY ? HOUR : DAY;
	return step - (elapsed % step);
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
	if (status === null) return null;
	if (status.kind === 'refreshing') return copy.refreshing();
	if (status.kind === 'never') return copy.never();
	const age = ageOf(status.at, now);
	if (age.unit === 'now') return copy.justNow();
	const when = new Intl.RelativeTimeFormat(locale, { numeric: 'always', style: 'short' }).format(
		-age.count,
		age.unit
	);
	return copy.updated(when);
}

/** One phrasing of the status line, and whether it is the one on show. */
export interface StatusPhrase {
	text: string;
	shown: boolean;
}

/**
 * The phrasings the status line stacks in one cell: the current label,
 * shown, and — while a refresh runs — the label it had before, hidden.
 * At rest the line is exactly as wide as its words, so every gap on the
 * header's line is the same; during a refresh the held label keeps the
 * line at its old width, so the Refresh action the user just pressed
 * does not slide out from under the pointer or the focus ring. `null`
 * (no tracker connected) has no phrasings.
 */
export function statusPhrases(
	status: WatchLaterStatus | null,
	now: number,
	locale: string,
	copy: StatusCopy,
	held: string | null = null
): StatusPhrase[] {
	void status;
	void now;
	void locale;
	void copy;
	void held;
	throw new Error('not implemented');
}
