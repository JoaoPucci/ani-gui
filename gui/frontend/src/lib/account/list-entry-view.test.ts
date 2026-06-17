import { describe, expect, it } from 'vitest';
import {
	deriveListEntryView,
	editorInitial,
	listButtonLabel,
	pickSeedEntry
} from './list-entry-view';
import {
	STATUS_OPTIONS,
	buildListEdit,
	clampProgress,
	effectiveProgress,
	effectiveStatus,
	statusOptionsFor
} from './list-entry-edit';
import type { EntryView } from './types';

// A trivial status labeller for deterministic label assertions.
const labels = {
	add: 'Add to list',
	statusLabel: (s: string) => s.charAt(0).toUpperCase() + s.slice(1)
};

describe('deriveListEntryView', () => {
	it('not on list → onList false, no status, progress 0', () => {
		const v = deriveListEntryView(null, 24);
		expect(v).toEqual({ onList: false, status: null, progress: 0, total: 24 });
	});

	it('mid-show → carries the live status + progress + total', () => {
		const entry: EntryView = { status: 'watching', progress: 5 };
		expect(deriveListEntryView(entry, 24)).toEqual({
			onList: true,
			status: 'watching',
			progress: 5,
			total: 24
		});
	});

	it('unknown total is preserved as null', () => {
		expect(deriveListEntryView({ status: 'watching', progress: 5 }, null).total).toBeNull();
	});
});

describe('listButtonLabel', () => {
	it('not on list → the add label', () => {
		expect(listButtonLabel(deriveListEntryView(null, 24), labels)).toBe('Add to list');
	});

	it('on list with a total → "Status · n/total"', () => {
		const v = deriveListEntryView({ status: 'watching', progress: 5 }, 24);
		expect(listButtonLabel(v, labels)).toBe('Watching · 5/24');
	});

	it('on list without a known total → "Status · n"', () => {
		const v = deriveListEntryView({ status: 'watching', progress: 5 }, null);
		expect(listButtonLabel(v, labels)).toBe('Watching · 5');
	});

	it('completed shows the full count', () => {
		const v = deriveListEntryView({ status: 'completed', progress: 24 }, 24);
		expect(listButtonLabel(v, labels)).toBe('Completed · 24/24');
	});
});

describe('pickSeedEntry', () => {
	it('all providers absent → null (truly not on any list)', () => {
		expect(pickSeedEntry([null, null])).toBeNull();
	});

	it('empty list → null', () => {
		expect(pickSeedEntry([])).toBeNull();
	});

	it('single tracked provider → that entry', () => {
		const e: EntryView = { status: 'watching', progress: 5 };
		expect(pickSeedEntry([e])).toEqual(e);
	});

	it('one provider absent, another tracked → the tracked one (never seed Add over real data)', () => {
		const e: EntryView = { status: 'watching', progress: 12 };
		expect(pickSeedEntry([null, e])).toEqual(e);
		expect(pickSeedEntry([e, null])).toEqual(e);
	});

	it('diverging progress → the furthest-along entry (writing it back never lowers a tracker)', () => {
		const a: EntryView = { status: 'watching', progress: 3 };
		const b: EntryView = { status: 'watching', progress: 12 };
		expect(pickSeedEntry([a, b])).toEqual(b);
		expect(pickSeedEntry([b, a])).toEqual(b);
	});

	it('equal progress → the more-committed status wins (completed over watching)', () => {
		const watching: EntryView = { status: 'watching', progress: 12 };
		const completed: EntryView = { status: 'completed', progress: 12 };
		expect(pickSeedEntry([watching, completed])).toEqual(completed);
		expect(pickSeedEntry([completed, watching])).toEqual(completed);
	});
});

describe('buildListEdit', () => {
	it('a tracker without the row (current null) is created with the chosen status', () => {
		expect(
			buildListEdit({ current: null, seededStatus: 'planning', status: 'planning', progress: 0 })
		).toEqual({ status: 'planning', progress: 0 });
	});

	it('creating a row at planning with positive progress promotes it to watching', () => {
		// The Add path (current null) must promote too: a started title left at
		// the default Planning would otherwise be created Plan-to-Watch with
		// watched episodes and linger in Watch Later.
		expect(
			buildListEdit({ current: null, seededStatus: 'planning', status: 'planning', progress: 5 })
		).toEqual({ status: 'watching', progress: 5 });
	});

	it('an existing row with an unchanged status omits status (each tracker keeps its own)', () => {
		// The seed picks one tracker's status; if the user only adjusts the
		// episode count, status must NOT be written or a divergent
		// rewatching/paused/dropped state on another tracker gets clobbered.
		expect(
			buildListEdit({
				current: 'completed',
				seededStatus: 'completed',
				status: 'completed',
				progress: 12
			})
		).toEqual({ progress: 12 });
	});

	it('a changed status is sent (a deliberate convergence)', () => {
		expect(
			buildListEdit({
				current: 'completed',
				seededStatus: 'completed',
				status: 'watching',
				progress: 12
			})
		).toEqual({ status: 'watching', progress: 12 });
	});

	it('promotes a planning row to watching when saving positive progress without a status change', () => {
		// Seed came from another tracker as watching; the planning tracker only
		// gets progress, which would leave it Plan-to-Watch with watched
		// episodes. Promote it to watching (the explicit /set path skips the
		// mark-watched promotion the auto path applies).
		expect(
			buildListEdit({
				current: 'planning',
				seededStatus: 'watching',
				status: 'watching',
				progress: 6
			})
		).toEqual({ status: 'watching', progress: 6 });
	});

	it('does not promote a planning row at zero progress', () => {
		expect(
			buildListEdit({
				current: 'planning',
				seededStatus: 'planning',
				status: 'planning',
				progress: 0
			})
		).toEqual({ progress: 0 });
	});

	it('always carries progress (the count converges per explicit-edits-win)', () => {
		expect(
			buildListEdit({
				current: 'watching',
				seededStatus: 'watching',
				status: 'watching',
				progress: 7
			}).progress
		).toBe(7);
	});
});

describe('effectiveStatus', () => {
	it('promotes planning to watching at positive progress', () => {
		expect(effectiveStatus('planning', 5)).toBe('watching');
	});
	it('leaves planning at zero progress', () => {
		expect(effectiveStatus('planning', 0)).toBe('planning');
	});
	it('leaves any non-planning status untouched', () => {
		expect(effectiveStatus('paused', 5)).toBe('paused');
		expect(effectiveStatus('completed', 12)).toBe('completed');
	});
});

describe('effectiveProgress', () => {
	it('forces a completed entry to the full episode count (status wins over a partial)', () => {
		expect(effectiveProgress('completed', 5, 12)).toBe(12);
		expect(effectiveProgress('completed', 0, 12)).toBe(12);
	});

	it('leaves a completed entry on its given count when the total is unknown', () => {
		expect(effectiveProgress('completed', 5, null)).toBe(5);
	});

	it('leaves non-completed statuses on the given count', () => {
		expect(effectiveProgress('watching', 5, 12)).toBe(5);
		expect(effectiveProgress('paused', 0, 12)).toBe(0);
	});
});

describe('buildListEdit — completed stays at the full count', () => {
	it('snaps progress up to total instead of writing a partial completed', () => {
		expect(
			buildListEdit({
				current: 'completed',
				seededStatus: 'completed',
				status: 'completed',
				progress: 0,
				total: 12
			})
		).toEqual({ progress: 12 });
	});

	it('keeps a completed provider full even when the chosen status matches a stale seed', () => {
		// Smoke-test repro: the tracker is really completed, the app seed was a
		// stale watching, and the user "kept" watching. Status matches the seed
		// so it's omitted — but the provider's real status is completed, so the
		// progress we write must snap to the full count rather than leave it at
		// completed/0.
		expect(
			buildListEdit({
				current: 'completed',
				seededStatus: 'watching',
				status: 'watching',
				progress: 0,
				total: 12
			})
		).toEqual({ progress: 12 });
	});

	it('snaps to total when the user newly marks a provider completed', () => {
		expect(
			buildListEdit({
				current: 'watching',
				seededStatus: 'watching',
				status: 'completed',
				progress: 3,
				total: 12
			})
		).toEqual({ status: 'completed', progress: 12 });
	});
});

describe('editorInitial', () => {
	it('seeds from the current entry when on the list', () => {
		const v = deriveListEntryView({ status: 'paused', progress: 7 }, 24);
		expect(editorInitial(v)).toEqual({ status: 'paused', progress: 7 });
	});

	it('defaults to planning / 0 when not on the list', () => {
		expect(editorInitial(deriveListEntryView(null, 24))).toEqual({
			status: 'planning',
			progress: 0
		});
	});

	it('shows a completed entry at the full count even if it was stored short', () => {
		// A pre-existing completed/0 (e.g. from a divergent write) must open at
		// the full count, not a locked 0 — completed always means all episodes.
		expect(editorInitial(deriveListEntryView({ status: 'completed', progress: 0 }, 12))).toEqual({
			status: 'completed',
			progress: 12
		});
	});
});

describe('statusOptionsFor', () => {
	it('a finished show offers every status', () => {
		expect(statusOptionsFor(false, null)).toEqual(STATUS_OPTIONS);
	});

	it('an airing show hides Completed and Rewatching (you cannot finish an unfinished show)', () => {
		expect(statusOptionsFor(true, null)).toEqual(['planning', 'watching', 'paused', 'dropped']);
	});

	it('an airing show keeps a status the entry is already set to (e.g. completed elsewhere)', () => {
		expect(statusOptionsFor(true, 'completed')).toEqual([
			'planning',
			'watching',
			'completed',
			'paused',
			'dropped'
		]);
		expect(statusOptionsFor(true, 'rewatching')).toEqual([
			'planning',
			'watching',
			'paused',
			'dropped',
			'rewatching'
		]);
	});
});

describe('clampProgress', () => {
	it('keeps an in-range count as-is', () => {
		expect(clampProgress(5, 24)).toBe(5);
		expect(clampProgress(24, 24)).toBe(24);
		expect(clampProgress(0, 24)).toBe(0);
	});

	it('caps at the episode total — no confirming an episode the show does not have', () => {
		expect(clampProgress(30, 24)).toBe(24);
		expect(clampProgress(99999, 12)).toBe(12);
	});

	it('floors at zero (a step below 0 or a negative entry)', () => {
		expect(clampProgress(-1, 24)).toBe(0);
		expect(clampProgress(-50, 24)).toBe(0);
	});

	it('coerces a non-finite entry (empty/NaN input) to zero', () => {
		expect(clampProgress(Number.NaN, 24)).toBe(0);
		expect(clampProgress(Number.POSITIVE_INFINITY, 24)).toBe(0);
	});

	it('floors a fractional count to a whole episode', () => {
		expect(clampProgress(5.9, 24)).toBe(5);
	});

	it('leaves the count uncapped when the total is unknown (ongoing show)', () => {
		expect(clampProgress(500, null)).toBe(500);
	});
});
