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

	it('pre-fills a pending status and snaps progress coherent to it (completed → full count)', () => {
		// After a partial save the live view reflects the still-lagging tracker's
		// value; reopening pre-fills the intended status, and progress must be made
		// coherent with THAT status — just like pickStatus does for in-place edits.
		const pending: PendingEdit = { kitsuId: 'k1', intendedStatus: 'completed' };
		expect(seedForOpen(view('watching', 5), pending, 'k1')).toEqual({
			status: 'completed',
			seededStatus: 'watching',
			progress: 24
		});
	});

	it('zeroes progress when the pending status is planning (else effectiveStatus re-promotes it)', () => {
		// The Codex P2 #3444274176 case: a reconcile leaves `live` at the lagging
		// tracker's watching/5, but the user intended Plan-to-Watch. Seeding
		// planning + progress 5 would promote back to watching on save, so the
		// retry never writes planning. Coerce progress to 0.
		const pending: PendingEdit = { kitsuId: 'k1', intendedStatus: 'planning' };
		expect(seedForOpen(view('watching', 5), pending, 'k1')).toEqual({
			status: 'planning',
			seededStatus: 'watching',
			progress: 0
		});
	});

	it('keeps progress for a pending status that carries a count (watching)', () => {
		const pending: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching' };
		expect(seedForOpen(view('paused', 7), pending, 'k1')).toEqual({
			status: 'watching',
			seededStatus: 'paused',
			progress: 7
		});
	});

	it('ignores a pending edit belonging to a different show', () => {
		const pending: PendingEdit = { kitsuId: 'other', intendedStatus: 'watching' };
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
	it('records the intended status on a partial save that changed status', () => {
		expect(pendingAfterSave(null, 'k1', 'partial', true, 'watching')).toEqual({
			kitsuId: 'k1',
			intendedStatus: 'watching'
		});
	});

	it('does NOT record a pending edit when a progress-only partial fails', () => {
		// statusChanged was false (the user only adjusted the episode count), so a
		// retry must stay progress-only — recording a status intent would later
		// force-write the seed status to every tracker and clobber a divergent
		// one (e.g. MAL paused → watching). (Codex P2 #3442488294)
		expect(pendingAfterSave(null, 'k1', 'partial', false, 'watching')).toBeNull();
	});

	it('refreshes the intended status on a repeat same-show status partial', () => {
		// The user changed status again while the popover stayed open after the
		// first partial; the pending intent must track the LATEST edit so a later
		// dismiss→reopen→retry propagates the newest status. (Codex P2 #3442360116)
		const prev: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'partial', true, 'paused')).toEqual({
			kitsuId: 'k1',
			intendedStatus: 'paused'
		});
	});

	it('replaces a pending edit from a different show on a status partial', () => {
		const prev: PendingEdit = { kitsuId: 'old', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'partial', true, 'completed')).toEqual({
			kitsuId: 'k1',
			intendedStatus: 'completed'
		});
	});

	it('clears the pending edit on a clean save (all trackers agree)', () => {
		const prev: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'saved', true, 'watching')).toBeNull();
	});

	it('leaves the pending edit untouched on a failed or noop save', () => {
		const prev: PendingEdit = { kitsuId: 'k1', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'failed', true, 'watching')).toBe(prev);
		expect(pendingAfterSave(prev, 'k1', 'noop', true, 'watching')).toBe(prev);
	});
});
