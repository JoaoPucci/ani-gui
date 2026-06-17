import { describe, expect, it } from 'vitest';
import {
	buildListEdit,
	deriveListEntryView,
	editorInitial,
	listButtonLabel,
	pickSeedEntry
} from './list-entry-view';
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
	it('adding a new entry sends status (creates with the chosen status)', () => {
		expect(
			buildListEdit({ onList: false, seededStatus: 'planning', status: 'planning', progress: 0 })
		).toEqual({ status: 'planning', progress: 0 });
	});

	it('on-list with an unchanged status omits status (each tracker keeps its own)', () => {
		// The seed picks one tracker's status; if the user only adjusts the
		// episode count, status must NOT be written or a divergent
		// rewatching/paused/dropped state on another tracker gets clobbered.
		expect(
			buildListEdit({ onList: true, seededStatus: 'completed', status: 'completed', progress: 12 })
		).toEqual({ progress: 12 });
	});

	it('on-list with a changed status sends it (a deliberate convergence)', () => {
		expect(
			buildListEdit({ onList: true, seededStatus: 'completed', status: 'watching', progress: 12 })
		).toEqual({ status: 'watching', progress: 12 });
	});

	it('always carries progress (the count converges per explicit-edits-win)', () => {
		expect(
			buildListEdit({ onList: true, seededStatus: 'watching', status: 'watching', progress: 7 })
				.progress
		).toBe(7);
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
});
