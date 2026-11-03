/**
 * Continue Watching's hand-offs.
 * Part of the mockups' views; `./view` gathers them.
 */
import { MOCK_RUNS } from './fixtures';
import { sequence, type WatchOrder } from '../order';
import type { MockEntry, MockRun } from './types';
import { upNextFor } from './view';

export interface Handoff {
	readonly run: MockRun;
	readonly finished: MockEntry;
	readonly next: MockEntry;
}

/**
 * Continue Watching's hand-offs: the last watched entry of each run
 * whose single next entry has aired and has not been started.
 */
export function handoffs(
	order: WatchOrder,
	today: string,
	runs: readonly MockRun[] = MOCK_RUNS
): Handoff[] {
	const out: Handoff[] = [];
	for (const run of runs) {
		const watched = sequence(run, order).filter((e) => run.progress[e.id] === 'watched');
		const finished = watched.at(-1);
		if (!finished) continue;
		const up = upNextFor(run, finished.id, order, today);
		if (up.kind === 'one' && run.progress[up.entry.id] === undefined) {
			out.push({ run, finished, next: up.entry });
		}
	}
	return out;
}
