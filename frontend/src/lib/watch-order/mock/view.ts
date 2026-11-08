/**
 * What the mockup surfaces show, derived from the fixtures: which run
 * an entry belongs to, its progress, whether it has aired; the
 * adapters live beside it and are re-exported here.
 */
import { MOCK_RUNS } from './fixtures';
import type { MockEntry, MockRun } from './types';

export function runFor(id: string, runs: readonly MockRun[] = MOCK_RUNS): MockRun | null {
	return runs.find((r) => r.entries.some((e) => e.id === id)) ?? null;
}

export function entryIn(run: MockRun, id: string): MockEntry | null {
	const all = [...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps];
	return all.find((e) => e.id === id) ?? null;
}

export { currentIn } from '../strip-view';
export { isUnaired, progressOf, progressShare, yearOf, type ProgressView } from '../entry';
export { asRef } from '../poster-ref';
export { franchiseName } from '../franchise-name';
