/**
 * What the mockup surfaces show, derived from the fixtures: which run
 * an entry belongs to, its progress, whether it has aired, the "Up
 * next" answer at a finale; the Continue hand-offs and the adapters
 * live beside it and are re-exported here.
 */
import { MOCK_RUNS } from './fixtures';
import { isUnaired } from '../entry';
import { placement, type WatchOrder } from '../order';
import type { MockEntry, MockRun } from './types';

export function runFor(id: string, runs: readonly MockRun[] = MOCK_RUNS): MockRun | null {
	return runs.find((r) => r.entries.some((e) => e.id === id)) ?? null;
}

export function entryIn(run: MockRun, id: string): MockEntry | null {
	const all = [...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps];
	return all.find((e) => e.id === id) ?? null;
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

export { handoffs, type Handoff } from './handoff';
export { currentIn } from '../strip-view';
export { isUnaired, progressOf, progressShare, yearOf, type ProgressView } from '../entry';
export { asRef, franchiseName, scrollLeftFor } from './adapters';
