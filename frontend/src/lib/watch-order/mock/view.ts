/**
 * What the mockup surfaces show, derived from the fixtures.
 * Signatures only until the readings land.
 */
import type { KitsuAnimeRef } from '$lib/api';
import type { WatchOrder } from './order';
import type { MockEntry, MockRun } from './types';

export function runFor(id: string, runs: readonly MockRun[] = []): MockRun | null {
	void id;
	void runs;
	return null;
}

export function entryIn(run: MockRun, id: string): MockEntry | null {
	void run;
	void id;
	return null;
}

export type ProgressView =
	| { kind: 'none' }
	| { kind: 'watched' }
	| { kind: 'partway'; episode: number; total: number | null };

export function progressOf(run: MockRun, entry: MockEntry): ProgressView {
	void run;
	void entry;
	return { kind: 'none' };
}

export function progressShare(view: ProgressView): number {
	void view;
	return 0;
}

export function isUnaired(entry: MockEntry, today: string): boolean {
	void entry;
	void today;
	return false;
}

export type UpNext =
	| { kind: 'one'; entry: MockEntry }
	| { kind: 'unaired'; entry: MockEntry }
	| { kind: 'choices'; entries: readonly MockEntry[] }
	| { kind: 'none' };

export function upNextFor(run: MockRun, id: string, order: WatchOrder, today: string): UpNext {
	void run;
	void id;
	void order;
	void today;
	return { kind: 'none' };
}

export interface Handoff {
	readonly run: MockRun;
	readonly finished: MockEntry;
	readonly next: MockEntry;
}

export function handoffs(
	order: WatchOrder,
	today: string,
	runs: readonly MockRun[] = []
): Handoff[] {
	void order;
	void today;
	void runs;
	return [];
}

export function currentIn(
	run: MockRun,
	order: WatchOrder
): { entry: MockEntry; episode: number } | null {
	void run;
	void order;
	return null;
}

export function yearOf(entry: MockEntry): string | null {
	void entry;
	return null;
}

export function asRef(entry: MockEntry): KitsuAnimeRef {
	return {
		id: entry.id,
		canonical_title: '',
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: null,
		status: null,
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

export function scrollLeftFor(
	box: { clientWidth: number; scrollWidth: number },
	itemStart: number,
	itemWidth: number,
	align: number
): number {
	void box;
	void itemStart;
	void itemWidth;
	void align;
	return 0;
}

export function franchiseName(title: string): string {
	return title;
}
