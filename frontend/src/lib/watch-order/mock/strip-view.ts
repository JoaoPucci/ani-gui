/**
 * What the show page's watch-order strip shows for one entry: its
 * place in its run, or, for a side story, the stories Kitsu names as
 * its parents. Part of the mockups' views.
 */
import { MOCK_RUNS } from './fixtures';
import type { WatchOrder } from './by-start';
import { placement } from './place';
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
	return (
		runs.find((r) => r.entries.some((e) => e.id === id)) ??
		runs.find((r) => r.parents[id] !== undefined) ??
		null
	);
}

export function stripView(run: MockRun, id: string, order: WatchOrder): StripView {
	const parents = run.parents[id];
	if (parents === undefined) return { kind: 'run', ...placement(run, id, order) };
	const all = [...run.entries, ...run.versions, ...run.films, ...run.sides];
	return {
		kind: 'side',
		position: null,
		total: null,
		before: parents.map((p) => all.find((e) => e.id === p)!),
		after: [],
		inCycle: false
	};
}

/** The text an empty column carries. */
export function emptyColumn(view: StripView, side: 'before' | 'after'): EmptyColumn {
	if (side === 'before') return 'first';
	return view.kind === 'side' ? 'side_end' : 'last';
}

/** The counts behind the "Whole franchise" control. */
export interface FranchiseSummary {
	/** Every entry the franchise page lists, recaps included. */
	readonly total: number;
	readonly main: number;
	/** Films and other versions. */
	readonly films: number;
	readonly sides: number;
	/** Up to three posters to fan out, from different parts of the franchise. */
	readonly posters: readonly MockEntry[];
}

export function franchiseSummary(run: MockRun): FranchiseSummary {
	const first = run.entries.find((e) => e.id === run.nameId);
	const candidates = [
		first,
		run.films[0] ?? run.versions[0],
		run.sides[0],
		...run.entries.filter((e) => e !== first)
	];
	const posters: MockEntry[] = [];
	for (const c of candidates) {
		if (c && !posters.includes(c) && posters.length < 3) posters.push(c);
	}
	return {
		total: 0,
		main: run.entries.length,
		films: run.films.length + run.versions.length,
		sides: run.sides.length,
		posters
	};
}

/**
 * The entry the viewer is part-way through, for the strip's "You're
 * watching" line: null when it is the entry being viewed or already
 * one of the strip's tiles, which shows its progress itself.
 */
export function watchingFor(
	run: MockRun,
	id: string,
	view: StripView,
	order: WatchOrder
): { entry: MockEntry; episode: number } | null {
	void run;
	void id;
	void view;
	void order;
	return null;
}
