/**
 * What the show page's watch-order strip shows for one entry: its
 * place in its run, or, for a side story, the stories Kitsu names as
 * its parents.
 */
import type { WatchOrder } from './by-start';
import { placement } from './place';
import { sequence } from './sequence';
import type { Progress, WatchEntry, WatchRun } from './types';

export interface StripView {
	/** `run`: a member of the run; `side`: a side story beside it. */
	readonly kind: 'run' | 'side';
	readonly position: number | null;
	readonly total: number | null;
	readonly before: readonly WatchEntry[];
	readonly after: readonly WatchEntry[];
	readonly inCycle: boolean;
}

/** What an empty column says. */
export type EmptyColumn = 'first' | 'last' | 'side_end';

export function stripView(run: WatchRun, id: string, order: WatchOrder): StripView {
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
	readonly recaps: number;
	/** Up to three entries with a poster to fan out, from different parts of the franchise. */
	readonly posters: readonly WatchEntry[];
}

export function franchiseSummary(run: WatchRun): FranchiseSummary {
	const first = run.entries.find((e) => e.id === run.nameId);
	const candidates = [
		first,
		run.films[0] ?? run.versions[0],
		run.sides[0],
		...run.entries.filter((e) => e !== first)
	];
	const posters: WatchEntry[] = [];
	for (const c of candidates) {
		if (c && c.poster !== null && !posters.includes(c) && posters.length < 3) posters.push(c);
	}
	return {
		total:
			run.entries.length +
			run.versions.length +
			run.films.length +
			run.sides.length +
			run.recaps.length,
		main: run.entries.length,
		films: run.films.length + run.versions.length,
		sides: run.sides.length,
		recaps: run.recaps.length,
		posters
	};
}

/** The entry the viewer is on: the latest started, not finished, in order. */
export function currentIn(
	run: WatchRun,
	progress: Progress,
	order: WatchOrder
): { entry: WatchEntry; episode: number } | null {
	const partway = sequence(run, order).filter((e) => typeof progress[e.id] === 'number');
	const last = partway.at(-1);
	return last ? { entry: last, episode: progress[last.id] as number } : null;
}

/**
 * The entry the viewer is part-way through, for the strip's "You're
 * watching" line: null when it is the entry being viewed or already
 * one of the strip's tiles, which shows its progress itself.
 */
export function watchingFor(
	run: WatchRun,
	progress: Progress,
	id: string,
	view: StripView,
	order: WatchOrder
): { entry: WatchEntry; episode: number } | null {
	const now = currentIn(run, progress, order);
	if (!now || now.entry.id === id) return null;
	const shown = [...view.before, ...view.after].some((e) => e.id === now.entry.id);
	return shown ? null : now;
}

/**
 * The entries "Similar titles" leaves out while the strip shows: the
 * run's own entries, which the strip and its franchise link stand for,
 * and the strip's tiles (a side story's parents).
 */
export function similarHidden(run: WatchRun, view: StripView): ReadonlySet<string> {
	return new Set([...run.entries, ...view.before, ...view.after].map((e) => e.id));
}
