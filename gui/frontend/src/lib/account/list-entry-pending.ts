// A per-show "pending edit" that survives the editor being dismissed and
// reopened after a partial multi-tracker save.
//
// When a save lands on some trackers but not others (a 429, say), the editor
// keeps the popover open so a retry re-sends the status to the laggards — but
// only while it stays open. The display value (`live`) is updated to what
// actually landed, and the editor's seed is derived from it, so once the user
// dismisses and reopens, the seed has moved to the landed value and
// `buildListEdit` treats the status as unchanged — the failed tracker never
// gets it. The pending edit decouples the two: it remembers the ORIGINAL
// pre-save seed (the divergence point) and the intended status across
// dismiss/reopen, so a retry still writes status to the trackers that lag.
//
// Pure + unit-tested; the component holds one slot keyed by kitsuId (the
// detail page shows one show at a time) and clears it on a clean save/remove.

import type { ListStatus } from './types';
import { editorInitial, type ListEntryView } from './list-entry-view';

export interface PendingEdit {
	/** The show this pending edit belongs to. */
	kitsuId: string;
	/** The seed the editor opened on BEFORE the partial save — the divergence
	 *  point a retry must compare against, not the landed value. */
	seededStatus: ListStatus;
	/** The status the user wants every tracker to reach. */
	intendedStatus: ListStatus;
}

/**
 * The values to open the editor with for `kitsuId`. With no surviving pending
 * edit, seed status and seed-status both from the live view (a progress-only
 * edit then leaves a divergent status alone). With a pending edit for THIS
 * show, re-apply the intended status but keep the original pre-save seed, so a
 * retry's {@link buildListEdit} still treats the status as deliberately
 * changed and re-sends it to the tracker that failed. Progress always comes
 * from the live view — it rides along on every per-tracker write regardless.
 */
export function seedForOpen(
	view: ListEntryView,
	pending: PendingEdit | null,
	kitsuId: string
): { status: ListStatus; seededStatus: ListStatus; progress: number } {
	const base = editorInitial(view);
	if (pending && pending.kitsuId === kitsuId) {
		return {
			status: pending.intendedStatus,
			seededStatus: pending.seededStatus,
			progress: base.progress
		};
	}
	return { status: base.status, seededStatus: base.status, progress: base.progress };
}

/**
 * The pending edit after a save outcome:
 *   - `partial`: record the pre-save seed + intended status so a dismissed,
 *     reopened retry still re-sends the status. A repeat partial for the SAME
 *     show keeps the existing record — advancing the seed to the (now landed)
 *     value would lose the still-divergent tracker; a partial for a DIFFERENT
 *     show replaces it (one slot, current show wins).
 *   - `saved`: every tracker accepted it, so the divergence is gone — clear.
 *   - `failed` / `noop`: nothing changed, leave the record as-is.
 */
export function pendingAfterSave(
	prev: PendingEdit | null,
	kitsuId: string,
	outcome: 'noop' | 'saved' | 'partial' | 'failed',
	seededStatus: ListStatus,
	intendedStatus: ListStatus
): PendingEdit | null {
	if (outcome === 'saved') return null;
	if (outcome === 'partial') {
		// Same show, popover still open: keep the ORIGINAL pre-save seed (advancing
		// it to the now-landed value would lose the still-divergent tracker) but
		// refresh the intended status to the user's latest pick, so a later retry
		// propagates the newest edit rather than a stale one.
		if (prev && prev.kitsuId === kitsuId) {
			return { kitsuId, seededStatus: prev.seededStatus, intendedStatus };
		}
		return { kitsuId, seededStatus, intendedStatus };
	}
	return prev;
}
