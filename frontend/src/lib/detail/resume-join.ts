/**
 * When the detail page reads its resume row again: once the row has
 * arrived and an availability lookup has answered. Split from
 * resume-refresh.ts for the per-file complexity bar.
 */

import type { HistoryEntry } from '$lib/api';

/** Runs `refresh` on the resume row once both have happened — the row
 *  has arrived and the page's availability lookup has answered — in
 *  either order, and again for each later lookup that answers
 *  (`unresolved` as it goes out) and for each other answer that can
 *  put the row right (`again`: a warm of the show, a re-probe).
 *  `reset` starts a new visit, with no refresh out. */
export function joinResumeRefresh(refresh: (entry: HistoryEntry) => unknown): {
	row: (entry: HistoryEntry | null) => void;
	resolved: () => void;
	unresolved: () => void;
	reset: () => void;
	settled: () => Promise<void>;
	refreshing: () => boolean;
	again: () => void;
} {
	let row: HistoryEntry | null = null;
	let answered = false;
	let ran = false;
	let pending: Promise<unknown> = Promise.resolve();
	let out = false;
	let again = false;
	const fire = () => {
		if (ran || !answered || !row) return;
		ran = true;
		out = true;
		const run: Promise<unknown> = Promise.resolve(refresh(row))
			.catch(() => {})
			.finally(() => {
				if (pending !== run) return;
				out = false;
				// Asked again while it was out: once more, now.
				if (again) {
					again = false;
					ran = false;
					fire();
				}
			});
		pending = run;
	};
	return {
		row: (entry) => {
			row = entry;
			fire();
		},
		resolved: () => {
			answered = true;
			fire();
		},
		unresolved: () => {
			answered = false;
			ran = false;
		},
		settled: () => pending.then(() => {}),
		refreshing: () => out,
		again: () => {
			// A lookup out runs the refresh when it answers.
			if (!answered) return;
			if (out) {
				again = true;
				return;
			}
			ran = false;
			fire();
		},
		reset: () => {
			row = null;
			answered = false;
			ran = false;
			// A refresh the last visit left out is not this one's.
			pending = Promise.resolve();
			out = false;
			again = false;
		}
	};
}

/** Waits while `join` has a refresh out, newer ones included, for at
 *  most `ms`; whether none is out by then. */
export async function settledWithin(
	join: { refreshing: () => boolean; settled: () => Promise<void> },
	ms: number,
	isCurrent: () => boolean = () => true
): Promise<boolean> {
	const deadline = Date.now() + ms;
	while (join.refreshing()) {
		const left = deadline - Date.now();
		if (left <= 0 || !isCurrent()) return false;
		// In slices, so a visit that ends stops the wait.
		await Promise.race([join.settled(), new Promise((r) => setTimeout(r, Math.min(left, 100)))]);
	}
	return isCurrent();
}
