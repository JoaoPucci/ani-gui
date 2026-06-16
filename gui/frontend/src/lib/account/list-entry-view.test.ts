import { describe, expect, it } from 'vitest';
import { deriveListEntryView, editorInitial, listButtonLabel } from './list-entry-view';
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

describe('editorInitial', () => {
	it('seeds from the current entry when on the list', () => {
		const v = deriveListEntryView({ status: 'paused', progress: 7 }, 24);
		expect(editorInitial(v)).toEqual({ status: 'paused', progress: 7 });
	});

	it('defaults to planning / 0 when not on the list', () => {
		expect(editorInitial(deriveListEntryView(null, 24))).toEqual({ status: 'planning', progress: 0 });
	});
});
