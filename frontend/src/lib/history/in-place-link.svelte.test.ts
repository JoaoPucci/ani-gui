// The link session of a surface that stays where it is — the player
// of a guessed session, Home's cards — against fake app dependencies.

import { beforeEach, describe, expect, it } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import type { LinkNotice } from './link-session.svelte';
import {
	InPlaceLinkSession,
	__resetHiddenRowsForTests,
	type InPlaceLinkDeps
} from './in-place-link.svelte';
import { notePick, noteSwitch, pickedRow } from './guess-slip';

function ref(id: string, title: string): KitsuAnimeRef {
	return {
		id,
		canonical_title: title,
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: 26,
		average_rating: null,
		subtype: 'TV',
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

const CARD: HistoryEntry = {
	id: 'hianime:jojos-bizarre-adventure-2000-5182',
	title: "JoJo's Bizarre Adventure",
	ep_no: '1'
};
const GUESS = ref('7158', 'JoJo 2012');
const RIGHT = ref('4013', 'JoJo 2000');

interface Fake {
	deps: InPlaceLinkDeps;
	notices: LinkNotice[];
	refreshed: number;
	rows: HistoryEntry[];
}

/** The app's dependencies; `inPlayer` for the player's session, which
 *  says what a pick did in its slip rather than in a notice. */
function fake(inPlayer = false): Fake {
	const f = { notices: [], refreshed: 0, rows: [CARD] } as unknown as Fake;
	f.deps = {
		history: async () => f.rows,
		resolve: async (e) =>
			e.kitsu_id
				? { match: ref(e.kitsu_id, 'linked'), trusted: true }
				: { match: GUESS, trusted: false },
		search: async () => [GUESS, RIGHT],
		group: async (_card, rows) => rows,
		link: async (_card, rows, kitsuId) => {
			f.rows = f.rows.map((r) => (rows.includes(r.id) ? { ...r, kitsu_id: kitsuId } : r));
			return {
				kitsu_id: kitsuId,
				linked: rows.map((row) => ({ row, previous: null, ep_no: '1', watched_at: null }))
			};
		},
		unlink: async (_k, rows) => {
			f.rows = f.rows.map((r) =>
				rows.some((l) => l.row === r.id) ? { ...r, kitsu_id: undefined } : r
			);
			return { restored: rows.map((r) => r.row), kept: [] };
		},
		notify: (n) => f.notices.push(n),
		refresh: () => f.refreshed++,
		inPlayer
	};
	return f;
}

beforeEach(() => __resetHiddenRowsForTests());

describe('the slip', () => {
	it('holds its room while the offer is decided, then shows the guess', async () => {
		const s = new InPlaceLinkSession(fake().deps);
		const arriving = s.arrive(CARD.id, GUESS.id);
		expect(s.slip).toBe('holding');
		await arriving;
		expect(s.slip).toBe('guess');
	});

	it('shows nothing for a session no guess opened, or a row that is no guess', async () => {
		const s = new InPlaceLinkSession(fake().deps);
		await s.arrive(null, GUESS.id);
		expect(s.slip).toBe('none');
		await s.arrive(CARD.id, RIGHT.id);
		expect(s.slip).toBe('none');
	});
});

describe('Hide', () => {
	it('sets the row aside for every session until the app restarts', async () => {
		const first = new InPlaceLinkSession(fake().deps);
		await first.arrive(CARD.id, GUESS.id);
		first.hide();
		expect(first.slip).toBe('none');
		// The offer stands: the ⋯ entry still reaches the picker.
		expect(first.offer).not.toBeNull();

		const again = new InPlaceLinkSession(fake().deps);
		await again.arrive(CARD.id, GUESS.id);
		expect(again.slip).toBe('none');

		__resetHiddenRowsForTests();
		const restarted = new InPlaceLinkSession(fake().deps);
		await restarted.arrive(CARD.id, GUESS.id);
		expect(restarted.slip).toBe('guess');
	});

	it('is per row', async () => {
		const f = fake();
		const other: HistoryEntry = { ...CARD, id: 'hianime:other-1' };
		f.rows = [CARD, other];
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		s.hide();
		await s.arrive(other.id, GUESS.id);
		expect(s.slip).toBe('guess');
	});
});

describe('"Yes" on the player', () => {
	it('records the show playing on the line without a word, and leaves the slip', async () => {
		const f = fake(true);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.answerYes(true);
		expect(f.rows[0].kitsu_id).toBe(GUESS.id);
		expect(s.slip).toBe('none');
		expect(s.linked).toBeNull();
		expect(f.notices).toEqual([]);
		expect(f.refreshed).toBe(1);
		// The ⋯ entry still changes the show, and asks nothing.
		expect(s.offer?.saved).toBe(GUESS.id);
	});

	it('records nothing where the play landed on another listing, and sets the slip aside', async () => {
		const f = fake(true);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.answerYes(false);
		expect(f.rows[0].kitsu_id).toBeUndefined();
		expect(s.slip).toBe('none');
		expect(f.notices).toEqual([]);
		expect(s.offer).not.toBeNull();
	});

	it('does nothing without an offer', async () => {
		const f = fake(true);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(null, GUESS.id);
		await s.answerYes(true);
		expect(f.notices).toEqual([]);
	});
});

describe('a pick on Home', () => {
	it('of the show on the card says it was saved, with an Undo that gives the guess back', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: GUESS, current: true });
		await s.confirm();
		expect(s.slip).toBe('none');
		const notice = f.notices.at(-1)!;
		expect(notice.message).toBe(m.detail_link_done_toast({ title: GUESS.canonical_title }));
		expect(notice.actionLabel).toBe(m.detail_link_undo());
		notice.onAction!();
		await new Promise((r) => setTimeout(r, 0));
		await new Promise((r) => setTimeout(r, 0));
		expect(f.rows[0].kitsu_id).toBeUndefined();
		expect(s.slip).toBe('guess');
	});

	it('keeps no offer once the card is saved, as the card is read again', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		expect(s.offer).toBeNull();
		expect(pickedRow(CARD.id)).toBe(false);
	});
});

describe('a pick of another show on the player', () => {
	it('records it, keeps watching what plays, and says so in the slip alone', async () => {
		const f = fake(true);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		expect(f.rows[0].kitsu_id).toBe(RIGHT.id);
		expect(s.slip).toBe('linked');
		expect(s.linked).toEqual({ id: RIGHT.id, title: RIGHT.canonical_title, show: RIGHT });
		// The confirmation is the slip's: no notice, so no Undo.
		expect(f.notices).toEqual([]);
		expect(f.refreshed).toBe(1);
	});

	it('keeps the ⋯ entry to change the show again, the one playing still named as such', async () => {
		const f = fake(true);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		expect(s.offer?.current.id).toBe(GUESS.id);
		expect(s.offer?.saved).toBe(RIGHT.id);
		expect(pickedRow(CARD.id)).toBe(true);
		// Picking the show playing after all: nothing left to switch to.
		await s.start(2);
		s.choose({ ref: GUESS, current: true });
		await s.confirm();
		expect(f.rows[0].kitsu_id).toBe(GUESS.id);
		expect(s.linked).toBeNull();
		expect(s.slip).toBe('none');
		expect(s.offer?.saved).toBe(GUESS.id);
		expect(f.notices).toEqual([]);
	});

	it('says whether the pick was saved, so a switch never follows one that was not', async () => {
		const f = fake(true);
		// The session keeps the deps it was built with: the save fails by
		// a switch it reads at each call.
		let down = false;
		const link = f.deps.link;
		f.deps.link = (card, rows, id) =>
			down ? Promise.reject(new Error('down')) : link(card, rows, id);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		expect(await s.confirm()).toBe(true);
		// A second pick that fails: the first one's switch stays as it was.
		down = true;
		await s.start(2);
		s.choose({ ref: ref('9999', 'JoJo 2007'), current: false });
		expect(await s.confirm()).toBe(false);
		expect(s.linked?.id).toBe(RIGHT.id);
	});

	it('leaves the slip as it was when the pick cannot be saved', async () => {
		const f = fake(true);
		f.deps.link = async () => Promise.reject(new Error('down'));
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		expect(await s.confirm()).toBe(false);
		expect(s.slip).toBe('guess');
		expect(s.linked).toBeNull();
		expect(s.error).not.toBeNull();
		expect(pickedRow(CARD.id)).toBe(false);
	});
});

describe('the session a switch opens', () => {
	it('says once which show plays now, and keeps the ⋯ entry to change it again', async () => {
		const f = fake(true);
		f.rows = [{ ...CARD, kitsu_id: RIGHT.id }];
		notePick(CARD.id);
		noteSwitch(CARD.id, RIGHT.canonical_title);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, RIGHT.id);
		expect(s.slip).toBe('switched');
		expect(s.switchedTitle).toBe(RIGHT.canonical_title);
		expect(s.offer?.current.id).toBe(RIGHT.id);
		expect(s.offer?.saved).toBe(RIGHT.id);
		// Once: the next arrival of the row is told nothing.
		const later = new InPlaceLinkSession(f.deps);
		await later.arrive(CARD.id, RIGHT.id);
		expect(later.slip).toBe('none');
		expect(later.offer).not.toBeNull();
	});

	it('offers nothing for a row no pick on the player saved', async () => {
		const f = fake(true);
		f.rows = [{ ...CARD, kitsu_id: RIGHT.id }];
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, RIGHT.id);
		expect(s.offer).toBeNull();
		expect(s.slip).toBe('none');
	});

	it('offers nothing once the row records another show than the one playing', async () => {
		const f = fake(true);
		f.rows = [{ ...CARD, kitsu_id: '9999' }];
		notePick(CARD.id);
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, RIGHT.id);
		expect(s.offer).toBeNull();
	});
});

describe('a surface that reads nothing from the history', () => {
	it('links and undoes without a refresh', async () => {
		const f = fake();
		delete f.deps.refresh;
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		expect(s.slip).toBe('linked');
		await s.undo(
			{
				kitsu_id: RIGHT.id,
				linked: [{ row: CARD.id, previous: null, ep_no: '1', watched_at: null }]
			},
			GUESS.id,
			CARD.id
		);
		expect(s.slip).toBe('guess');
	});
});

describe('a "Yes" that fails', () => {
	it('says so in a notice, since the picker that shows errors is closed', async () => {
		const f = fake(true);
		f.deps.link = async () => Promise.reject(new Error('down'));
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.answerYes(true);
		expect(f.notices.at(-1)).toEqual({ kind: 'error', message: m.detail_link_failed() });
		expect(s.slip).toBe('guess');
	});
});

describe('a later arrival', () => {
	it("forgets the earlier offer's rows, so a link links the new card's", async () => {
		const f = fake();
		const other: HistoryEntry = { ...CARD, id: 'hianime:other-1' };
		f.rows = [CARD, other];
		f.deps.group = async (card) => [card];
		const linked: string[][] = [];
		const link = f.deps.link;
		f.deps.link = async (card, rows, id) => {
			linked.push(rows);
			return link(card, rows, id);
		};
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		await new Promise((r) => setTimeout(r, 0));
		expect(s.group).toEqual([CARD.id]);
		await s.arrive(other.id, GUESS.id);
		expect(s.group).toBeNull();
		await s.confirmCurrent();
		expect(linked.at(-1)).toEqual([other.id]);
	});
});
