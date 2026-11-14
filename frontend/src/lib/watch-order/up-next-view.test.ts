import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import type { WatchEntry } from './types';
import { franchiseCardMeta, upNextAnnouncement, upNextKey } from './up-next-view';

const e = (id: string): WatchEntry => ({
	id,
	title: `T${id}`,
	subtype: 'TV',
	start: '2020-01-01',
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});
const summary = { total: 7, main: 3, films: 4, sides: 0, recaps: 0, posters: [] };

describe('the Up next card copy', () => {
	it('keys each form of the card', () => {
		expect(upNextKey({ kind: 'one', entry: e('1') })).toBe(m.watchorder_upnext_eyebrow());
		expect(upNextKey({ kind: 'unaired', entry: e('1') })).toBe(m.watchorder_upnext_coming());
		expect(upNextKey({ kind: 'choices', entries: [e('1'), e('2')] })).toBe(
			m.watchorder_upnext_choices()
		);
		expect(upNextKey(null)).toBe(m.watchorder_upnext_franchise_key());
	});

	it('announces the first title, or the whole franchise by its name', () => {
		expect(upNextAnnouncement({ kind: 'choices', entries: [e('1'), e('2')] }, null)).toBe(
			m.watchorder_upnext_announce({ title: 'T1' })
		);
		expect(upNextAnnouncement(null, { name: 'Fate/stay night', id: '1', summary })).toBe(
			m.watchorder_upnext_franchise_announce({ name: 'Fate/stay night' })
		);
	});

	it("gives the franchise card the first two parts the strip's link names", () => {
		expect(franchiseCardMeta(summary)).toBe(
			[
				m.watchorder_strip_franchise_stack_main({ count: 3 }),
				m.watchorder_strip_franchise_stack_films({ count: 4 })
			].join(' · ')
		);
	});
});
