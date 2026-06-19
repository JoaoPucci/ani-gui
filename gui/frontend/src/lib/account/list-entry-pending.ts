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

export interface PendingEdit {
	/** The show this pending edit belongs to. */
	kitsuId: string;
	/** The status the user wants every tracker to reach. */
	intendedStatus: ListStatus;
}

/**
 * The values to open the editor with for `kitsuId`. `status` pre-fills the
 * picker — the intended status when a pending edit for this show survives,
 * else the live view's status. `seededStatus` is the opened-on value (always
 * the live view) used to detect a deliberate status change in the no-pending
 * case. Progress always comes from the live view (it rides along on every
 * per-tracker write regardless).
 */
export function seedForOpen(
	view: ListEntryView,
	pending: PendingEdit | null,
	kitsuId: string
): { status: ListStatus; seededStatus: ListStatus; progress: number } {
	const base = editorInitial(view);
	const status = pending && pending.kitsuId === kitsuId ? pending.intendedStatus : base.status;
	return { status, seededStatus: base.status, progress: base.progress };
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
 *   - `partial`: record the intended status (latest pick) so a dismissed,
 *     reopened retry still re-sends it. A repeat partial for the same show
 *     just refreshes the intent; a partial for a different show replaces it
 *     (one slot, current show wins).
 *   - `saved`: every tracker accepted it, so the divergence is gone — clear.
 *   - `failed` / `noop`: nothing changed, leave the record as-is.
 */
export function pendingAfterSave(
	prev: PendingEdit | null,
	kitsuId: string,
	outcome: 'noop' | 'saved' | 'partial' | 'failed',
	intendedStatus: ListStatus
): PendingEdit | null {
	if (outcome === 'saved') return null;
	if (outcome === 'partial') return { kitsuId, intendedStatus };
	return prev;
}
