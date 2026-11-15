import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import type { WatchEntry } from './types';
import {
	franchiseCardLabel,
	franchiseCardMeta,
	cardAccentId,
	cardAnnouncement,
	cardKey
} from './up-next-view';
import type { PlayerUpNext } from './chain-end';

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
const entryCard = (upNext: Extract<PlayerUpNext, { kind: 'entry' }>['card']['upNext']) =>
	({ kind: 'entry', card: { upNext, progress: {} } }) as const;
const franchiseCard: PlayerUpNext = {
	kind: 'franchise',
	card: { name: 'Fate/stay night', id: '9', front: '3', summary }
};

describe('the Up next card copy', () => {
	it('keys each form of the card', () => {
		expect(cardKey(entryCard({ kind: 'one', entry: e('1') }))).toBe(m.watchorder_upnext_eyebrow());
		expect(cardKey(entryCard({ kind: 'unaired', entry: e('1') }))).toBe(
			m.watchorder_upnext_coming()
		);
		expect(cardKey(entryCard({ kind: 'choices', entries: [e('1'), e('2')] }))).toBe(
			m.watchorder_upnext_choices()
		);
		expect(cardKey(franchiseCard)).toBe(m.watchorder_upnext_franchise_key());
	});

	it('announces the first title, or the whole franchise by its name', () => {
		expect(cardAnnouncement(entryCard({ kind: 'choices', entries: [e('1'), e('2')] }))).toBe(
			m.watchorder_upnext_announce({ title: 'T1' })
		);
		expect(cardAnnouncement(franchiseCard)).toBe(
			m.watchorder_upnext_franchise_announce({ name: 'Fate/stay night' })
		);
	});

	it("labels the franchise card's link with its key and the franchise's name", () => {
		expect(franchiseCard.kind === 'franchise' && franchiseCardLabel(franchiseCard.card)).toBe(
			m.watchorder_upnext_franchise_label({ name: 'Fate/stay night' })
		);
	});

	it("takes the accent from the first title named, or the franchise's front entry", () => {
		expect(cardAccentId(entryCard({ kind: 'choices', entries: [e('4'), e('2')] }))).toBe('4');
		expect(cardAccentId(franchiseCard)).toBe('3');
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
