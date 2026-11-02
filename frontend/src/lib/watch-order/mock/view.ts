/**
 * What the mockup surfaces show, derived from the fixtures: which run
 * an entry belongs to, its progress, whether it has aired, the "Up
 * next" answer at a finale, and the Continue hand-offs.
 */
import { MOCK_RUNS } from './fixtures';
import { placement, sequence, type WatchOrder } from './order';
import type { KitsuAnimeRef } from '$lib/api';
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

/** The entry the viewer is on: the latest started, not finished, in order. */
export function currentIn(
	run: MockRun,
	order: WatchOrder
): { entry: MockEntry; episode: number } | null {
	const partway = sequence(run, order).filter((e) => typeof run.progress[e.id] === 'number');
	const last = partway.at(-1);
	return last ? { entry: last, episode: run.progress[last.id] as number } : null;
}

/** "2023" from a start date, or null. */
export function yearOf(entry: MockEntry): string | null {
	return entry.start?.slice(0, 4) ?? null;
}

/** A fixture entry in the shape PosterCard takes. */
export function asRef(entry: MockEntry): KitsuAnimeRef {
	return {
		id: entry.id,
		canonical_title: entry.title,
		slug: null,
		synopsis: null,
		start_date: entry.start,
		end_date: null,
		episode_count: entry.episodes,
		average_rating: null,
		subtype: entry.subtype,
		status: entry.status,
		age_rating: null,
		popularity_rank: null,
		poster_image: entry.poster
			? { tiny: null, small: null, medium: entry.poster, large: null, original: null }
			: null,
		cover_image: entry.cover
			? { tiny: null, small: null, large: entry.cover, original: null }
			: null
	};
}

/**
 * The scroll offset that brings an item into a horizontal scroller at
 * `align` of the free width (0 the start, 0.5 the middle, 1 the end),
 * clamped to the scrollable range. `itemStart` is measured from the
 * start of the scroller's content. The rail puts the entry being
 * viewed late in the window, so what came before it stays in sight.
 */
export function scrollLeftFor(
	box: { clientWidth: number; scrollWidth: number },
	itemStart: number,
	itemWidth: number,
	align: number
): number {
	const target = itemStart - (box.clientWidth - itemWidth) * align;
	return Math.max(0, Math.min(target, box.scrollWidth - box.clientWidth));
}

/** A franchise's name from its first entry: Kitsu's "(TV)"-style format suffix dropped. */
export function franchiseName(title: string): string {
	return title.replace(/\s*\((TV|Movie|OVA|ONA|Special)\)$/i, '');
}
