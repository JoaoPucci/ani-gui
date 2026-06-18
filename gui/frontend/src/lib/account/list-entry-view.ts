// Pure read/display derivation for the detail-page list control: fold the
// live current entries into the seed, and turn the seed + episode total into
// the action-row button label and the editor's initial values. i18n is
// injected so this stays pure + unit-tested (the component passes the
// localized status names + "Add to list" label). Write-side helpers (status
// options, coherence, clamping) live in `list-entry-edit.ts`.

import type { EntryView, ListStatus } from './types';
import { effectiveProgress } from './list-entry-edit';

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
	// A completed entry always reads as the full count (status wins over a
	// stored-short progress), so the button + editor agree with what a Save
	// would write — never "Completed · 0/24".
	return {
		onList: true,
		status: entry.status,
		progress: effectiveProgress(entry.status, entry.progress, kitsuTotal),
		total: kitsuTotal
	};
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
	// Plan to Watch means not started — show just the status, no count.
	if (view.status === 'planning') return labels.statusLabel(view.status);
	const count = view.total !== null ? `${view.progress}/${view.total}` : `${view.progress}`;
	return `${labels.statusLabel(view.status)} · ${count}`;
}

/**
 * Initial editor values: seed from the live entry when on the list,
 * else default to a fresh Plan-to-Watch at episode 0.
 */
export function editorInitial(view: ListEntryView): { status: ListStatus; progress: number } {
	const status = view.status ?? 'planning';
	// A completed entry opens at the full count, never a stored-short value —
	// the editor then locks the episode field while Completed is selected.
	return { status, progress: effectiveProgress(status, view.progress, view.total) };
}
