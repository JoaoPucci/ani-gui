// Pure derivation for the detail-page list control: turn the live
// current entry (or its absence) + the show's episode total into the
// action-row button label and the editor's initial values. i18n is
// injected so this stays a pure, unit-tested function (the component
// passes the localized status names + "Add to list" label).

import type { EntryView, ListStatus } from './types';

/** The status options the editor offers, in display order. */
export const STATUS_OPTIONS: ListStatus[] = [
	'planning',
	'watching',
	'completed',
	'paused',
	'dropped',
	'rewatching'
];

// How "committed" each status is, for breaking ties when two trackers
// sit at the same episode count. A higher rank means further along the
// watch lifecycle, so on a tie we seed from the more-engaged entry.
const STATUS_COMMITMENT: Record<ListStatus, number> = {
	planning: 0,
	dropped: 1,
	paused: 2,
	watching: 3,
	rewatching: 4,
	completed: 5
};

/**
 * Fold the live current entry read from every connected provider into the
 * single entry the editor seeds from. The detail page reads each tracker
 * but a Save fans the result out to ALL of them, so seeding from one
 * provider that lacks the title (→ Add/Planning/0) would let a Save clobber
 * another tracker that already has progress. Picking the furthest-along
 * entry — greatest progress, tie-broken by the more-committed status —
 * means writing the seed back never lowers a tracker below where it was.
 * Returns null only when no provider has the show on its list.
 */
export function pickSeedEntry(entries: (EntryView | null)[]): EntryView | null {
	let best: EntryView | null = null;
	for (const e of entries) {
		if (!e) continue;
		if (
			best === null ||
			e.progress > best.progress ||
			(e.progress === best.progress && STATUS_COMMITMENT[e.status] > STATUS_COMMITMENT[best.status])
		) {
			best = e;
		}
	}
	return best;
}

export interface ListEntryView {
	/** Whether the show is on the user's list. */
	onList: boolean;
	/** Current unified status, or null when not on the list. */
	status: ListStatus | null;
	/** Episodes watched so far (0 when not on the list). */
	progress: number;
	/** The show's full episode total, or null when unknown. */
	total: number | null;
}

/**
 * Fold the live `EntryView` (from `getEntry`, `null` when not on the
 * list) + the Kitsu episode total into the view model the action row and
 * editor consume.
 */
export function deriveListEntryView(
	entry: EntryView | null,
	kitsuTotal: number | null
): ListEntryView {
	if (!entry) {
		return { onList: false, status: null, progress: 0, total: kitsuTotal };
	}
	return { onList: true, status: entry.status, progress: entry.progress, total: kitsuTotal };
}

export interface ListButtonLabels {
	/** Label when the show isn't on the list yet (e.g. "Add to list"). */
	add: string;
	/** Localized name for a status (e.g. 'watching' → "Watching"). */
	statusLabel: (status: ListStatus) => string;
}

/**
 * The action-row button text: the add label when not on the list, else
 * "Status · n/total" (or "Status · n" when the total is unknown).
 */
export function listButtonLabel(view: ListEntryView, labels: ListButtonLabels): string {
	if (!view.onList || view.status === null) return labels.add;
	const count = view.total !== null ? `${view.progress}/${view.total}` : `${view.progress}`;
	return `${labels.statusLabel(view.status)} · ${count}`;
}

/**
 * Initial editor values: seed from the live entry when on the list,
 * else default to a fresh Plan-to-Watch at episode 0.
 */
export function editorInitial(view: ListEntryView): { status: ListStatus; progress: number } {
	return {
		status: view.status ?? 'planning',
		progress: view.progress
	};
}

/**
 * Build the edit the editor's Save fans out to every connected tracker.
 * Progress always rides along (the count converges per explicit-edits-win).
 * Status is written only when it's meaningful: when adding a show (no
 * tracker has it, so there's no divergent state to clobber) or when the
 * user changed it from the seeded value. Otherwise status is omitted so a
 * Save that only touched the episode count leaves each tracker's own
 * status intact — the seed collapses divergent trackers to one status, and
 * blindly writing it back would wipe a deliberate rewatching/paused/dropped
 * state on another tracker.
 */
export function buildListEdit(opts: {
	onList: boolean;
	seededStatus: ListStatus;
	status: ListStatus;
	progress: number;
}): { status?: ListStatus; progress: number } {
	const edit: { status?: ListStatus; progress: number } = { progress: opts.progress };
	if (!opts.onList || opts.status !== opts.seededStatus) {
		edit.status = opts.status;
	}
	return edit;
}
