/**
 * What the mockup surfaces show, derived from the fixtures: which run
 * an entry belongs to, its progress, whether it has aired, the "Up
 * next" answer at a finale; the Continue hand-offs and the adapters
 * live beside it and are re-exported here.
 */
import { MOCK_RUNS } from './fixtures';
import { placement, type WatchOrder } from './order';
import type { MockEntry, MockRun } from './types';

export function runFor(id: string, runs: readonly MockRun[] = MOCK_RUNS): MockRun | null {
	return runs.find((r) => r.entries.some((e) => e.id === id)) ?? null;
}

export function entryIn(run: MockRun, id: string): MockEntry | null {
	const all = [...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps];
	return all.find((e) => e.id === id) ?? null;
}

export type ProgressView =
	| { kind: 'none' }
	| { kind: 'watched' }
	| { kind: 'partway'; episode: number; total: number | null };

export function progressOf(run: MockRun, entry: MockEntry): ProgressView {
	const p = run.progress[entry.id];
	if (p === undefined) return { kind: 'none' };
	if (p === 'watched') return { kind: 'watched' };
	return { kind: 'partway', episode: p, total: entry.episodes };
}

/** Share of the entry watched, 0–1, for a progress bar. */
export function progressShare(view: ProgressView): number {
	if (view.kind === 'watched') return 1;
	if (view.kind === 'partway' && view.total) return Math.min(1, view.episode / view.total);
	return 0;
}

/** Not aired yet: a future start, or no start at all. */
export function isUnaired(entry: MockEntry, today: string): boolean {
	return entry.start === null || entry.start > today;
}

export type UpNext =
	| { kind: 'one'; entry: MockEntry }
	| { kind: 'unaired'; entry: MockEntry }
	| { kind: 'choices'; entries: readonly MockEntry[] }
	| { kind: 'none' };

/** The proposal's end-card rule: one aired, one unaired, several, or none. */
export function upNextFor(run: MockRun, id: string, order: WatchOrder, today: string): UpNext {
	const after = placement(run, id, order).after;
	if (after.length === 0) return { kind: 'none' };
	if (after.length > 1) return { kind: 'choices', entries: after };
	const entry = after[0];
	return isUnaired(entry, today) ? { kind: 'unaired', entry } : { kind: 'one', entry };
}

/** "2023" from a start date, or null. */
export function yearOf(entry: MockEntry): string | null {
	return entry.start?.slice(0, 4) ?? null;
}

export { currentIn, handoffs, type Handoff } from './handoff';
export { asRef, franchiseName, scrollLeftFor } from './adapters';
