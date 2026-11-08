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
import { progressOf, progressShare } from './entry';
import { franchiseName } from './franchise-name';
import type { WatchOrderData } from './loader';
import { pathNote, placement, segments, sequence, type Segment } from './order';
import { progressFromHistory } from './progress';
import { shortTitle } from './short-title';
import { runOf } from './strip-model';
import { currentIn, franchiseSummary } from './strip-view';
import type { EntryProgress, Progress, WatchEntry, WatchRun } from './types';

/** One title of the main story. */
export interface FranchiseRow {
	readonly entry: WatchEntry;
	/** The title cut against the rest of the main story's. */
	readonly label: string;
	/** The oversized numeral, on numbered stretches only. */
	readonly position: number | null;
	readonly progress: EntryProgress | undefined;
	/** Share watched, 0–1, for the progress bar. */
	readonly share: number;
	/** The short title this one leads into, where it only leads into another. */
	readonly leadsInto: string | null;
	/** The title the page was opened from. */
	readonly here: boolean;
}

/** One part of the main story, as the page lays it out. */
export type FranchiseSection =
	| {
			readonly kind: 'line';
			readonly rows: readonly FranchiseRow[];
			/** Titles with no number and no note: they have no clear place in the order. */
			readonly unplaced: boolean;
	  }
	| { readonly kind: 'ways'; readonly ways: readonly (readonly FranchiseRow[])[] }
	| { readonly kind: 'group'; readonly rows: readonly FranchiseRow[] };

export interface FranchisePageModel {
	readonly order: WatchOrder;
	readonly run: WatchRun;
	/** The franchise's name: its first title, "(TV)" dropped where nothing clashes. */
	readonly name: string;
	/** The title the name comes from, whose artwork and accent the masthead takes. */
	readonly art: WatchEntry;
	/** Every title the page lists. */
	readonly total: number;
	/** The title the viewer is part-way through, where Resume goes. */
	readonly resume: {
		readonly entry: WatchEntry;
		readonly label: string;
		readonly episode: number;
		readonly position: number | null;
	} | null;
	/** Where "Start from the beginning" goes: the first title in the order. */
	readonly start: WatchEntry;
	readonly sections: readonly FranchiseSection[];
	readonly versions: readonly WatchEntry[];
	readonly films: readonly WatchEntry[];
	readonly sides: readonly WatchEntry[];
	readonly recaps: readonly WatchEntry[];
	/** The franchise read stopped early, so there may be more titles. */
	readonly partial: boolean;
}

/** What the rows of one page share: the run, the order, progress and short titles. */
interface RowContext {
	readonly run: WatchRun;
	readonly order: WatchOrder;
	readonly progress: Progress;
	readonly id: string;
	readonly label: (e: WatchEntry) => string;
}

function rowOf(ctx: RowContext, entry: WatchEntry, numbered: boolean): FranchiseRow {
	const progress = ctx.progress[entry.id];
	const note = pathNote(ctx.run, entry.id, ctx.order);
	return {
		entry,
		label: ctx.label(entry),
		position: numbered ? placement(ctx.run, entry.id, ctx.order).position : null,
		progress,
		share: progressShare(progressOf(entry, progress)),
		leadsInto: note?.kind === 'leads-into' ? ctx.label(note.entry) : null,
		here: entry.id === ctx.id
	};
}

/** A title the page can neither number nor say what it leads into. */
const unexplained = (r: FranchiseRow) => r.position === null && r.leadsInto === null;

/**
 * A line's titles as the page draws them: numbered titles (and titles
 * that say what they lead into) together, and each stretch of titles
 * with neither under its own "no clear place" heading.
 */
function lineSections(rows: readonly FranchiseRow[]): FranchiseSection[] {
	const out: { kind: 'line'; rows: FranchiseRow[]; unplaced: boolean }[] = [];
	for (const r of rows) {
		const last = out.at(-1);
		if (last && last.unplaced === unexplained(r)) last.rows.push(r);
		else out.push({ kind: 'line', rows: [r], unplaced: unexplained(r) });
	}
	return out;
}

/**
 * One segment as the page draws it. A fork left with one way (its
 * other ways start at a join or in a loop) is drawn as a line.
 */
function sectionsOf(ctx: RowContext, seg: Segment): FranchiseSection[] {
	if (seg.kind === 'fork' && seg.branches.length > 1)
		return [{ kind: 'ways', ways: seg.branches.map((b) => b.map((e) => rowOf(ctx, e, false))) }];
	if (seg.kind === 'cycle')
		return [{ kind: 'group', rows: seg.entries.map((e) => rowOf(ctx, e, false)) }];
	const entries = seg.kind === 'fork' ? seg.branches.flat() : seg.entries;
	return lineSections(entries.map((e) => rowOf(ctx, e, true)));
}

/** Stretches with no clear place that follow each other, under one heading. */
function joinUnplaced(sections: readonly FranchiseSection[]): FranchiseSection[] {
	const out: FranchiseSection[] = [];
	for (const s of sections) {
		const last = out.at(-1);
		if (s.kind === 'line' && s.unplaced && last?.kind === 'line' && last.unplaced)
			out[out.length - 1] = { ...last, rows: [...last.rows, ...s.rows] };
		else out.push(s);
	}
	return out;
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
		total: franchiseSummary(run).total,
		resume: now && {
			entry: now.entry,
			label: ctx.label(now.entry),
			episode: now.episode,
			position: placement(run, now.entry.id, order).position
		},
		start: sequence(run, order)[0],
		sections: joinUnplaced(segs.flatMap((seg) => sectionsOf(ctx, seg))),
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
