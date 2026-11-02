import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef } from '$lib/api';
import { arrive, emptyView, restoredView } from './arrival';
import { captureSearch, type SearchSnapshot } from './snapshot';

const ref = (id: string) => ({ id, canonical_title: id }) as KitsuAnimeRef;

function saved(q = 'frieren', withResults = true): SearchSnapshot {
	return captureSearch({
		q,
		order: 'year',
		subtypes: ['TV'],
		checkpoint: withResults
			? {
					mode: 'sub',
					fetchedAt: 0,
					hits: [ref('1'), ref('21')],
					seen: ['1', '2', '21'],
					next: 40,
					emptyStreak: 0,
					pages: 2,
					phase: 'idle'
				}
			: null,
		cards: [{ id: '21', top: -40, bottom: 240 }],
		now: 0
	});
}

describe('emptyView', () => {
	it('is the page with nothing asked: no query, Kitsu ranking, no filter, nothing to land on', () => {
		const view = emptyView();
		expect(view).toEqual({
			q: '',
			order: 'relevance',
			subtypes: [],
			anchor: null,
			restoredIds: new Set()
		});
	});
});

describe('restoredView', () => {
	it('puts back the saved query, order, filters and card, and holds back the saved cards', () => {
		const view = restoredView(saved());
		expect(view.q).toBe('frieren');
		expect(view.order).toBe('year');
		expect(view.subtypes).toEqual(['TV']);
		expect(view.anchor).toEqual({ id: '21', top: -40 });
		expect([...view.restoredIds]).toEqual(['1', '21']);
	});

	it('holds back no cards when no results were saved', () => {
		expect(restoredView(saved('frieren', false)).restoredIds.size).toBe(0);
	});

	it('does not share the saved filters', () => {
		const s = saved();
		restoredView(s).subtypes.push('movie');
		expect(s.subtypes).toEqual(['TV']);
	});
});

describe('arrive', () => {
	it('restores a saved search on a back or forward press onto its address', () => {
		const s = saved();
		expect(arrive(s, 'popstate', 'frieren')).toEqual({
			kind: 'restore',
			view: restoredView(s),
			saved: s,
			reload: false
		});
	});

	it('restores on a reload, saying it is one', () => {
		const out = arrive(saved(), 'enter', 'frieren');
		expect(out.kind === 'restore' && out.reload).toBe(true);
	});

	it('follows the address when the saved search is not the one it names', () => {
		expect(arrive(saved('himmel'), 'popstate', 'frieren')).toEqual({
			kind: 'follow',
			q: 'frieren'
		});
		expect(arrive(saved(), 'popstate', '')).toEqual({ kind: 'follow', q: '' });
	});

	it('follows the address on a back press onto an entry that saved nothing', () => {
		expect(arrive(null, 'popstate', 'frieren')).toEqual({ kind: 'follow', q: 'frieren' });
	});

	it('leaves any other navigation to the address it already followed', () => {
		for (const type of ['link', 'goto', 'form', 'enter']) {
			expect(arrive(null, type, 'frieren')).toEqual({ kind: 'none' });
		}
		expect(arrive(saved(), 'link', 'frieren')).toEqual({ kind: 'none' });
	});

	it('restores only the search the address names, and only on a press or a reload', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('frieren', 'himmel', ''),
				fc.constantFrom('frieren', 'himmel', ''),
				fc.boolean(),
				fc.constantFrom('popstate', 'enter', 'link', 'goto', 'form', 'leave'),
				(savedQ, q, has, type) => {
					const out = arrive(has ? saved(savedQ) : null, type, q);
					if (out.kind === 'restore') {
						expect(has && savedQ === q && q !== '').toBe(true);
						expect(['popstate', 'enter']).toContain(type);
						expect(out.reload).toBe(type === 'enter');
						expect(out.view.q).toBe(q);
					}
					if (out.kind === 'follow') expect(out.q).toBe(q);
				}
			)
		);
	});
});
