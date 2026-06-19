import { describe, it, expect } from 'vitest';
import {
	seedForOpen,
	pendingAfterSave,
	statusWriteIntended,
	type PendingEdit
} from './list-entry-pending';
import type { ListEntryView } from './list-entry-view';

const view = (
	status: ListEntryView['status'],
	progress = 0,
	total: number | null = 24
): ListEntryView => ({ onList: status !== null, status, progress, total });

describe('seedForOpen', () => {
	it('with no pending edit, seeds status AND seed from the live view', () => {
		// status === seededStatus, so a progress-only edit isn't a deliberate
		// status write (statusWriteIntended stays false).
		expect(seedForOpen(view('watching', 5), null, 'k1')).toEqual({
			status: 'watching',
			seededStatus: 'watching',
			progress: 5
		});
	});

	it('not on any list seeds a fresh Plan-to-Watch at 0', () => {
		expect(seedForOpen(view(null), null, 'k1')).toEqual({
			status: 'planning',
			seededStatus: 'planning',
			progress: 0
		});
	});

	it('reseeds a pending retry from the stored intent, not the folded live view', () => {
		// Codex P2 #3444295964: the user saved watching/5, MAL (already completed
		// at 24) rate-limited, and a reconcile folds the live view up to 24. The
		// retry must replay the user's intended 5, not the lagging tracker's 24.
		const pending: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching', intendedProgress: 5 };
		expect(seedForOpen(view('watching', 24), pending, 'k1')).toEqual({
			status: 'watching',
			seededStatus: 'watching',
			progress: 5
		});
	});

	it('coerces the stored progress coherent to the pending status (planning → 0)', () => {
		// Codex P2 #3444274176: planning + watched episodes would promote back to
		// watching on save, so a pending planning retry seeds at 0.
		const pending: PendingEdit = { kitsuId: 'k1', intendedStatus: 'planning', intendedProgress: 5 };
		expect(seedForOpen(view('watching', 5), pending, 'k1')).toEqual({
			status: 'planning',
			seededStatus: 'watching',
			progress: 0
		});
	});

	it('coerces the stored progress coherent to the pending status (completed → full count)', () => {
		const pending: PendingEdit = {
			kitsuId: 'k1',
			intendedStatus: 'completed',
			intendedProgress: 5
		};
		expect(seedForOpen(view('watching', 5), pending, 'k1')).toEqual({
			status: 'completed',
			seededStatus: 'watching',
			progress: 24
		});
	});

	it('ignores a pending edit belonging to a different show', () => {
		const pending: PendingEdit = { kitsuId: 'other', intendedStatus: 'watching', intendedProgress: 0 };
		expect(seedForOpen(view('completed', 24), pending, 'k1')).toEqual({
			status: 'completed',
			seededStatus: 'completed',
			progress: 24
		});
	});
});

describe('statusWriteIntended', () => {
	it('an active pending edit forces a status write even when the pick equals the seed', () => {
		// This is what lets a user REVERT a partially-landed status: reopening and
		// re-choosing the original value still writes it to the tracker that moved.
		expect(statusWriteIntended(true, 'planning', 'planning')).toBe(true);
	});

	it('without a pending edit, a status write is intended only when the pick differs from the seed', () => {
		expect(statusWriteIntended(false, 'watching', 'planning')).toBe(true);
		expect(statusWriteIntended(false, 'watching', 'watching')).toBe(false);
	});
});

describe('pendingAfterSave', () => {
	it('records the intended status AND progress on a partial save that changed status', () => {
		expect(pendingAfterSave(null, 'k1', 'partial', true, 'watching', 5)).toEqual({
			kitsuId: 'k1',
			intendedStatus: 'watching',
			intendedProgress: 5
		});
	});

	it('does NOT record a pending edit when a progress-only partial fails', () => {
		// statusChanged was false (episode-only edit): a retry must stay
		// progress-only and not force-write a status onto a divergent tracker.
		// (Codex P2 #3442488294)
		expect(pendingAfterSave(null, 'k1', 'partial', false, 'watching', 5)).toBeNull();
	});

	it('refreshes the intent on a repeat same-show status partial', () => {
		// (Codex P2 #3442360116)
		const prev: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching', intendedProgress: 5 };
		expect(pendingAfterSave(prev, 'k1', 'partial', true, 'paused', 8)).toEqual({
			kitsuId: 'k1',
			intendedStatus: 'paused',
			intendedProgress: 8
		});
	});

	it('replaces a pending edit from a different show on a status partial', () => {
		const prev: PendingEdit = { kitsuId: 'old', intendedStatus: 'watching', intendedProgress: 1 };
		expect(pendingAfterSave(prev, 'k1', 'partial', true, 'completed', 12)).toEqual({
			kitsuId: 'k1',
			intendedStatus: 'completed',
			intendedProgress: 12
		});
	});

	it('clears the pending edit on a clean save (all trackers agree)', () => {
		const prev: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching', intendedProgress: 5 };
		expect(pendingAfterSave(prev, 'k1', 'saved', true, 'watching', 5)).toBeNull();
	});

	it('leaves the pending edit untouched on a failed or noop save', () => {
		const prev: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching', intendedProgress: 5 };
		expect(pendingAfterSave(prev, 'k1', 'failed', true, 'watching', 5)).toBe(prev);
		expect(pendingAfterSave(prev, 'k1', 'noop', true, 'watching', 5)).toBe(prev);
	});
});
