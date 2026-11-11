/**
 * The fixture run whose strip an entry shows in the mockups: the one it
 * belongs to, or the one its parent stories belong to.
 */
import { MOCK_RUNS } from './fixtures';
import type { MockRun } from './types';

export function stripRunFor(id: string, runs: readonly MockRun[] = MOCK_RUNS): MockRun | null {
	return (
		runs.find((r) => r.entries.some((e) => e.id === id)) ??
		runs.find((r) => r.parents[id] !== undefined) ??
		null
	);
}
