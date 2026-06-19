import { describe, it, expect } from 'vitest';
import { seedForOpen, pendingAfterSave, type PendingEdit } from './list-entry-pending';
import type { ListEntryView } from './list-entry-view';

const view = (
	status: ListEntryView['status'],
	progress = 0,
	total: number | null = 24
): ListEntryView => ({ onList: status !== null, status, progress, total });

describe('seedForOpen', () => {
	it('with no pending edit, seeds status AND seed from the live view', () => {
		// The seed equals the opened-on status, so a progress-only edit doesn't
		// converge a divergent status (matches the pre-pending behaviour).
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

	it('with a pending edit for THIS show, re-seeds the intended status but keeps the ORIGINAL pre-save seed', () => {
		// After a partial save the live view reflects the landed value (watching),
		// but the seed must stay at the pre-save divergence point (planning) so a
		// retry's buildListEdit still treats the status as deliberately changed and
		// re-sends it to the tracker that failed.
		const pending: PendingEdit = {
			kitsuId: 'k1',
			seededStatus: 'planning',
			intendedStatus: 'watching'
		};
		expect(seedForOpen(view('watching', 5), pending, 'k1')).toEqual({
			status: 'watching',
			seededStatus: 'planning',
			progress: 5
		});
	});

	it('ignores a pending edit belonging to a different show', () => {
		const pending: PendingEdit = {
			kitsuId: 'other',
			seededStatus: 'planning',
			intendedStatus: 'watching'
		};
		expect(seedForOpen(view('completed', 24), pending, 'k1')).toEqual({
			status: 'completed',
			seededStatus: 'completed',
			progress: 24
		});
	});
});

describe('pendingAfterSave', () => {
	it('records the pre-save seed + intended status on a partial save', () => {
		expect(pendingAfterSave(null, 'k1', 'partial', 'planning', 'watching')).toEqual({
			kitsuId: 'k1',
			seededStatus: 'planning',
			intendedStatus: 'watching'
		});
	});

	it('keeps the existing pending edit on a repeated partial (never advances the seed)', () => {
		// A second partial must not move the recorded seed to the now-landed value,
		// or the still-divergent tracker is lost on the next retry.
		const prev: PendingEdit = { kitsuId: 'k1', seededStatus: 'planning', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'partial', 'watching', 'watching')).toBe(prev);
	});

	it('replaces a pending edit from a different show on a partial', () => {
		const prev: PendingEdit = { kitsuId: 'old', seededStatus: 'planning', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'partial', 'paused', 'completed')).toEqual({
			kitsuId: 'k1',
			seededStatus: 'paused',
			intendedStatus: 'completed'
		});
	});

	it('clears the pending edit on a clean save (all trackers agree)', () => {
		const prev: PendingEdit = { kitsuId: 'k1', seededStatus: 'planning', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'saved', 'planning', 'watching')).toBeNull();
	});

	it('leaves the pending edit untouched on a failed or noop save', () => {
		const prev: PendingEdit = { kitsuId: 'k1', seededStatus: 'planning', intendedStatus: 'watching' };
		expect(pendingAfterSave(prev, 'k1', 'failed', 'planning', 'watching')).toBe(prev);
		expect(pendingAfterSave(prev, 'k1', 'noop', 'planning', 'watching')).toBe(prev);
	});
});
