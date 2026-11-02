/**
 * What the show page's watch-order strip shows for one entry: its
 * place in its run, or, for a side story, the stories Kitsu names as
 * its parents. Part of the mockups' views.
 */
import { MOCK_RUNS } from './fixtures';
import type { WatchOrder } from './by-start';
import type { MockEntry, MockRun } from './types';

export interface StripView {
	/** `run`: a member of the run; `side`: a side story beside it. */
	readonly kind: 'run' | 'side';
	readonly position: number | null;
	readonly total: number | null;
	readonly before: readonly MockEntry[];
	readonly after: readonly MockEntry[];
	readonly inCycle: boolean;
}

/** What an empty column says. */
export type EmptyColumn = 'first' | 'last' | 'side_end';

/** The run whose strip an entry shows: the one it belongs to, or the one its parent stories belong to. */
export function stripRunFor(id: string, runs: readonly MockRun[] = MOCK_RUNS): MockRun | null {
	void id;
	void runs;
	return null;
}

export function stripView(run: MockRun, id: string, order: WatchOrder): StripView {
	void run;
	void id;
	void order;
	return { kind: 'run', position: null, total: null, before: [], after: [], inCycle: false };
}

/** The text an empty column carries. */
export function emptyColumn(view: StripView, side: 'before' | 'after'): EmptyColumn {
	void view;
	void side;
	return 'first';
}

/** The counts behind the "Whole franchise" control. */
export interface FranchiseSummary {
	readonly main: number;
	/** Films and other versions. */
	readonly films: number;
	readonly sides: number;
	/** Up to three posters to fan out, from different parts of the franchise. */
	readonly posters: readonly MockEntry[];
}

export function franchiseSummary(run: MockRun): FranchiseSummary {
	void run;
	return { main: 0, films: 0, sides: 0, posters: [] };
}
