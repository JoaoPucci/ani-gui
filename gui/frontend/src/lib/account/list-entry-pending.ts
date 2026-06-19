// A per-show "pending edit" that survives the editor being dismissed and
// reopened after a partial multi-tracker save.
//
// When a save lands on some trackers but not others (a 429, say), the editor
// keeps the popover open so a retry re-sends the status to the laggards — but
// only while it stays open. The display value (`live`) moves to what actually
// landed, and the editor seeds from it; so once the user dismisses and
// reopens, a plain Save looks like "no status change" and the failed tracker
// never catches up. The pending edit fixes that: it remembers the status the
// user intended and marks the next save as a deliberate status write (see
// statusWriteIntended), so a retry — including re-choosing the ORIGINAL status
// to undo the change — propagates to every tracker that still lags.
//
// Pure + unit-tested; the component holds one slot keyed by kitsuId (the
// detail page shows one show at a time) and clears it on a clean save/remove.

import type { ListStatus } from './types';
import { editorInitial, type ListEntryView } from './list-entry-view';
import { effectiveProgress } from './list-entry-edit';

export interface PendingEdit {
	/** The show this pending edit belongs to. */
	kitsuId: string;
	/** The status the user wants every tracker to reach. */
	intendedStatus: ListStatus;
	/** The episode count the user intended, captured at save time. Stored (not
	 *  re-derived from the post-partial live view) so a retry replays the user's
	 *  count rather than a lagging tracker's higher one folded back in. */
	intendedProgress: number;
}

/**
 * The values to open the editor with for `kitsuId`. When a pending edit for
 * this show survives, `status` and `progress` are seeded WHOLESALE from the
 * stored intent — never from the post-partial live view, which a reconcile can
 * fold up to a lagging tracker's higher count and overwrite the user's intended
 * progress. Otherwise both come from the live view. `seededStatus` is the
 * opened-on value (always the live view) used to detect a deliberate status
 * change in the no-pending case. `progress` is then made coherent with the
 * seeded `status` (planning → 0, completed → full count) exactly like
 * pickStatus does for in-place edits.
 */
export function seedForOpen(
	view: ListEntryView,
	pending: PendingEdit | null,
	kitsuId: string
): { status: ListStatus; seededStatus: ListStatus; progress: number } {
	const base = editorInitial(view);
	const match = pending !== null && pending.kitsuId === kitsuId;
	const status = match ? pending.intendedStatus : base.status;
	const rawProgress = match ? pending.intendedProgress : base.progress;
	return {
		status,
		seededStatus: base.status,
		progress: effectiveProgress(status, rawProgress, view.total)
	};
}

/**
 * Whether the editor's Save should write status to every tracker (vs. leave
 * each tracker's own status alone and write progress only). True when a
 * pending partial-save retry is active for this show — so re-choosing the
 * original status reverts the tracker that moved — OR the user moved the pick
 * off the opened-on value this session.
 */
export function statusWriteIntended(
	pendingActive: boolean,
	status: ListStatus,
	seededStatus: ListStatus
): boolean {
	return pendingActive || status !== seededStatus;
}

/**
 * The pending edit after a save outcome:
 *   - `partial` that wrote status (`statusChanged`): record the intended status
 *     (latest pick) so a dismissed, reopened retry still re-sends it. A repeat
 *     status partial for the same show refreshes the intent; one for a
 *     different show replaces it (one slot, current show wins).
 *   - `partial` that was progress-only (`!statusChanged`): leave the record
 *     as-is — there's no status intent to carry, so a retry stays progress-only
 *     and can't force-write the seed status onto a divergent tracker.
 *   - `saved`: every tracker accepted it, so the divergence is gone — clear.
 *   - `failed` / `noop`: nothing changed, leave the record as-is.
 */
export function pendingAfterSave(
	prev: PendingEdit | null,
	kitsuId: string,
	outcome: 'noop' | 'saved' | 'partial' | 'failed',
	statusChanged: boolean,
	intendedStatus: ListStatus,
	intendedProgress: number
): PendingEdit | null {
	if (outcome === 'saved') return null;
	if (outcome === 'partial' && statusChanged) {
		return { kitsuId, intendedStatus, intendedProgress };
	}
	return prev;
}
