import { describe, expect, test } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { kitsuGroupSiblingIds } from './delete-group';

// Stub builder so the test stays focused on the id-grouping logic
// without dragging every HistoryEntry field through each case.
function h(id: string, kitsu_id?: string): HistoryEntry {
	return {
		id,
		ep_no: '1',
		title: 'Stub',
		watched_at: 1,
		kitsu_id: kitsu_id ?? ''
	} as HistoryEntry;
}
function m(id: string): KitsuAnimeRef {
	return { id, canonical_title: 'Stub' } as KitsuAnimeRef;
}

describe('kitsuGroupSiblingIds', () => {
	test('returns every history id whose match resolves to the same Kitsu id', () => {
		const history = [h('aa-1'), h('aa-2'), h('aa-3')];
		const matches = {
			'aa-1': m('k-42'),
			'aa-2': m('k-42'),
			'aa-3': m('k-99')
		};
		expect(kitsuGroupSiblingIds('aa-1', history, matches).sort()).toEqual(['aa-1', 'aa-2']);
	});

	test('falls back to the lone id when the deleted entry has no resolved Kitsu match', () => {
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': null, 'aa-2': m('k-42') };
		expect(kitsuGroupSiblingIds('aa-1', history, matches)).toEqual(['aa-1']);
	});

	test('ignores siblings whose matches resolve to different Kitsu ids', () => {
		const history = [h('aa-1'), h('aa-2'), h('aa-3')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-2'), 'aa-3': m('k-1') };
		expect(kitsuGroupSiblingIds('aa-1', history, matches).sort()).toEqual(['aa-1', 'aa-3']);
	});

	test('handles a singleton group (no other entries share the Kitsu id)', () => {
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-2') };
		expect(kitsuGroupSiblingIds('aa-1', history, matches)).toEqual(['aa-1']);
	});
});
