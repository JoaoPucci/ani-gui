/**
 * The franchise page's main story as it is laid out: one row per
 * title, and the sections the rows stand in (numbered stretches, ways
 * to continue side by side, groups with no clear order, and titles
 * with no clear place). Part of the franchise page's model;
 * `./franchise-page` builds the rest.
 */
import type { WatchOrder } from './by-start';
import { progressOf, progressShare } from './entry';
import { gatherUnplaced } from './no-clear-place';
import { pathNote, placement, type Segment } from './order';
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

/** What the rows of one page share: the run, the order, progress and short titles. */
export interface RowContext {
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

/**
 * One segment as the page draws it. A fork left with one way (its
 * other ways start at a join or in a loop) is drawn as a line.
 */
function sectionOf(ctx: RowContext, seg: Segment): FranchiseSection {
	if (seg.kind === 'fork' && seg.branches.length > 1)
		return { kind: 'ways', ways: seg.branches.map((b) => b.map((e) => rowOf(ctx, e, false))) };
	if (seg.kind === 'cycle')
		return { kind: 'group', rows: seg.entries.map((e) => rowOf(ctx, e, false)) };
	const entries = seg.kind === 'fork' ? seg.branches.flat() : seg.entries;
	return { kind: 'line', rows: entries.map((e) => rowOf(ctx, e, true)), unplaced: false };
}

/**
 * The main story's sections, from the run's segments. The titles with
 * no clear place leave their lines and gather in one stretch at the
 * end, under its heading, so nothing numbered follows it.
 */
export function mainStory(ctx: RowContext, segs: readonly Segment[]): FranchiseSection[] {
	return gatherUnplaced(
		ctx,
		segs.map((seg) => sectionOf(ctx, seg))
	);
}
