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
	const fire = () => {
		if (ran || !answered || !row) return;
		ran = true;
		out = true;
		const run = Promise.resolve(refresh(row))
			.catch(() => {})
			.finally(() => {
				if (pending === run) out = false;
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
			answered = true;
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
		}
	};
}

/** Waits while `join` has a refresh out, newer ones included, for at
 *  most `ms`; whether none is out by then. */
export async function settledWithin(
	join: { refreshing: () => boolean; settled: () => Promise<void> },
	ms: number
): Promise<boolean> {
	const deadline = Date.now() + ms;
	while (join.refreshing()) {
		const left = deadline - Date.now();
		if (left <= 0) return false;
		await Promise.race([join.settled(), new Promise((r) => setTimeout(r, left))]);
	}
	return true;
}
