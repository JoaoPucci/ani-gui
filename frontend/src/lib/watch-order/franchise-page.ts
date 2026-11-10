/**
 * Everything the franchise page draws, in one pure reading of the
 * franchise payload and the watch history: the masthead (the name, its
 * artwork, the title count and where Resume goes), the main story as
 * numbered stretches, branches side by side and groups with no clear
 * order, and the rails around it. The route and its view only hand
 * these on.
 */
import type { BreadcrumbSegment } from '$lib/breadcrumb';
import { m } from '$lib/paraglide/messages';
import type { WatchOrder } from './by-start';
import { franchiseName } from './franchise-name';
import type { WatchOrderData } from './loader';
import { placement, segments, sequence } from './order';
import {
	mainStory,
	type FranchiseRow,
	type FranchiseSection,
	type RowContext
} from './franchise-sections';
import { progressFromHistory } from './progress';
import { shortTitle } from './short-title';
import { runOf } from './strip-model';
import { currentIn, franchiseSummary } from './strip-view';
import type { WatchEntry, WatchRun } from './types';

export type { FranchiseRow, FranchiseSection };

export interface FranchisePageModel {
	readonly order: WatchOrder;
	readonly run: WatchRun;
	/** The franchise's name: its first title, "(TV)" dropped where nothing clashes. */
	readonly name: string;
	/** The title the name comes from, whose artwork and accent the masthead takes. */
	readonly art: WatchEntry;
	/** Every title the page lists; null for a franchise read in part. */
	readonly total: number | null;
	/**
	 * The franchise is the show alone: it lists the show and nothing
	 * else, from a read whole.
	 */
	readonly alone: boolean;
	/** The title the viewer is part-way through, where Resume goes. */
	readonly resume: {
		readonly entry: WatchEntry;
		readonly label: string;
		readonly episode: number;
		readonly position: number | null;
	} | null;
	/**
	 * Where "Start from the beginning" goes: the first title in the
	 * order. Null for a franchise read in part, whose first title read
	 * may not be the first.
	 */
	readonly start: WatchEntry | null;
	readonly sections: readonly FranchiseSection[];
	readonly versions: readonly WatchEntry[];
	readonly films: readonly WatchEntry[];
	readonly sides: readonly WatchEntry[];
	readonly recaps: readonly WatchEntry[];
	/** The franchise read stopped early, so there may be more titles. */
	readonly partial: boolean;
}

/**
 * `section` without its numbers. The layout is made with them, so a
 * title keeps its stretch rather than moving into "No clear place".
 */
function unnumbered(section: FranchiseSection): FranchiseSection {
	const blank = (r: FranchiseRow): FranchiseRow => ({ ...r, position: null });
	if (section.kind === 'ways') return { ...section, ways: section.ways.map((w) => w.map(blank)) };
	return { ...section, rows: section.rows.map(blank) };
}

/** Whether `run` lists `id` and nothing else. */
function isAlone(run: WatchRun, id: string): boolean {
	const all = [...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps];
	return all.length === 1 && all[0].id === id;
}

/** The page for `id`, or null where there is no franchise to show. */
export function franchisePage(
	data: WatchOrderData,
	id: string,
	order: WatchOrder
): FranchisePageModel | null {
	if (data.franchise === null || data.franchise.anchor !== id) return null;
	const run = runOf(data.franchise);
	if (run === null) return null;
	const titles = run.entries.map((e) => e.title);
	const ctx: RowContext = {
		run,
		order,
		id,
		progress: progressFromHistory(data.rows, run.entries),
		label: (e) => shortTitle(e.title, titles)
	};
	// Read in part, the walk may have missed titles before the first one
	// it reached: nothing counted from that title is sure.
	const whole = data.franchise.complete;
	const art = run.entries.find((e) => e.id === run.nameId)!;
	const others = [...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps]
		.filter((e) => e.id !== art.id)
		.map((e) => e.title);
	const now = currentIn(run, ctx.progress, order);
	const segs = segments(run, order);
	return {
		order,
		run,
		name: franchiseName(art.title, others),
		art,
		total: whole ? franchiseSummary(run).total : null,
		alone: whole && isAlone(run, id),
		resume: now && {
			entry: now.entry,
			label: ctx.label(now.entry),
			episode: now.episode,
			position: whole ? placement(run, now.entry.id, order).position : null
		},
		start: whole ? sequence(run, order)[0] : null,
		sections: whole ? mainStory(ctx, segs) : mainStory(ctx, segs).map(unnumbered),
		versions: run.versions,
		films: run.films,
		sides: run.sides,
		recaps: run.recaps,
		partial: !data.franchise.complete
	};
}

/**
 * The topbar trail for the franchise page opened from `id`: Home, the
 * show it was opened from (a link back to it), then "Franchise".
 */
export function franchiseTrail(model: FranchisePageModel, id: string): BreadcrumbSegment[] {
	const r = model.run;
	const from = [...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps].find(
		(e) => e.id === id
	);
	return [
		{ label: m.breadcrumb_home(), href: '/' },
		...(from ? [{ label: from.title, href: `/anime/${id}` }] : []),
		{ label: m.watchorder_franchise_eyebrow() }
	];
}
