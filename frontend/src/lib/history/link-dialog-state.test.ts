// @vitest-environment happy-dom
// The change-show picker's decisions: where focus goes as it opens,
// as a show is picked, on Back and on close, and what each candidate
// row and the list say.

import { afterEach, describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef } from '$lib/api';
import type { LinkCandidate } from './link-candidates';
import { candidateMeta, createDialogFocus, listStatus } from './link-dialog-state';
import { m } from '$lib/paraglide/messages';
import { subtypeLabel } from '$lib/search/subtype-label';

afterEach(() => {
	document.body.innerHTML = '';
});

const button = () => {
	const b = document.createElement('button');
	document.body.append(b);
	return b;
};

const candidate = (over: Partial<KitsuAnimeRef> = {}): LinkCandidate => ({
	ref: {
		id: '1',
		canonical_title: 'Show',
		start_date: '2000-04-01',
		subtype: 'OVA',
		episode_count: 13,
		...over
	} as KitsuAnimeRef,
	current: false
});

describe('the picker opening', () => {
	it('remembers what had focus once per opening, never the page itself', () => {
		const focus = createDialogFocus();
		const pill = button();
		const other = button();
		focus.opened(pill, document.body);
		// Still open: what has focus now is inside the dialog, not its opener.
		focus.opened(other, document.body);
		expect(focus.closed()).toBe(pill);

		focus.opened(document.body, document.body);
		expect(focus.closed()).toBeNull();
	});
});

describe('the picker closing', () => {
	it("gives focus back to the opener while it is in the document, else to the surface's fallback", () => {
		const focus = createDialogFocus();
		const card = button();
		const pill = button();
		focus.opened(pill, document.body);
		pill.remove();
		expect(focus.closed(() => card)).toBe(card);
	});

	it('does nothing when it was not open', () => {
		const focus = createDialogFocus();
		const card = button();
		expect(focus.closed(() => card)).toBeNull();
	});

	it('forgets the opener and the show picked, for the next opening', () => {
		const focus = createDialogFocus();
		focus.opened(button(), document.body);
		focus.focusFor('7');
		focus.closed();
		focus.opened(document.body, document.body);
		expect(focus.focusFor(null)).toEqual({ to: 'search' });
		expect(focus.closed()).toBeNull();
	});
});

describe('focus inside the picker', () => {
	it('is the search field while nothing was picked', () => {
		expect(createDialogFocus().focusFor(null)).toEqual({ to: 'search' });
	});

	it('is the Link button once a show is picked', () => {
		expect(createDialogFocus().focusFor('7')).toEqual({ to: 'confirm' });
	});

	it("is the picked show's row on Back", () => {
		const focus = createDialogFocus();
		focus.focusFor('7');
		expect(focus.focusFor(null)).toEqual({ to: 'row', id: '7' });
		// Another pick, then Back: that one's row.
		focus.focusFor('9');
		expect(focus.focusFor(null)).toEqual({ to: 'row', id: '9' });
	});

	it('Back lands on the last show picked, whatever was picked before it', () => {
		fc.assert(
			fc.property(fc.array(fc.option(fc.constantFrom('1', '2', '3'), { nil: null })), (steps) => {
				const focus = createDialogFocus();
				let last: string | null = null;
				for (const s of steps) {
					focus.focusFor(s);
					if (s !== null) last = s;
				}
				expect(focus.focusFor(null)).toEqual(
					last === null ? { to: 'search' } : { to: 'row', id: last }
				);
			})
		);
	});
});

describe("a candidate row's facts", () => {
	it('are its year, its format and its episode count, in that order', () => {
		expect(candidateMeta(candidate())).toEqual([
			{ text: '2000', num: true },
			{ text: subtypeLabel('OVA'), num: false },
			{ text: `13 ${m.search_card_eps_suffix({ count: 13 })}`, num: true }
		]);
	});

	it('leave out a year and a count nobody knows, never the format', () => {
		expect(candidateMeta(candidate({ start_date: null, episode_count: null }))).toEqual([
			{ text: subtypeLabel('OVA'), num: false }
		]);
	});
});

describe('what the list says to assistive tech', () => {
	it('is loading, failed, empty, or nothing once there are shows', () => {
		expect(listStatus(null, false)).toBe(m.detail_link_candidates_loading());
		expect(listStatus([], true)).toBe(m.detail_link_candidates_failed());
		expect(listStatus([], false)).toBe(m.detail_link_candidates_empty());
		expect(listStatus([candidate()], false)).toBe('');
	});
});
