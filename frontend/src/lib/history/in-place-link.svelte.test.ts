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

function fake(): Fake {
	const f = { notices: [], refreshed: 0, rows: [CARD] } as unknown as Fake;
	f.deps = {
		history: async () => f.rows,
		resolve: async (e) =>
			e.kitsu_id
				? { match: ref(e.kitsu_id, 'linked'), trusted: true }
				: { match: GUESS, trusted: false },
		search: async () => [GUESS, RIGHT],
		group: async (_card, rows) => rows,
		link: async (rows, kitsuId) => {
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
		refresh: () => f.refreshed++
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

describe("That's right", () => {
	it('links the card to its guess, says it was kept, and leaves the slip', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.confirmCurrent();
		expect(f.rows[0].kitsu_id).toBe(GUESS.id);
		expect(s.slip).toBe('none');
		expect(s.linked).toBeNull();
		const notice = f.notices.at(-1)!;
		expect(notice.message).toBe(m.play_link_kept_toast({ title: GUESS.canonical_title }));
		expect(notice.actionLabel).toBe(m.detail_link_undo());
		expect(f.refreshed).toBe(1);
	});

	it('gives the guess back on Undo', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.confirmCurrent();
		f.notices.at(-1)!.onAction!();
		await new Promise((r) => setTimeout(r, 0));
		await new Promise((r) => setTimeout(r, 0));
		expect(f.rows[0].kitsu_id).toBeUndefined();
		expect(s.slip).toBe('guess');
	});

	it('does nothing without an offer', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(null, GUESS.id);
		await s.confirmCurrent();
		expect(f.notices).toEqual([]);
	});
});

describe('a link to another show', () => {
	it('notes the linked show, while what plays is still the guess', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		expect(s.slip).toBe('linked');
		expect(s.linked).toEqual({ id: RIGHT.id, title: RIGHT.canonical_title });
		expect(f.notices.at(-1)!.message).toBe(
			m.detail_link_done_toast({ title: RIGHT.canonical_title })
		);
		expect(f.refreshed).toBe(1);
	});

	it('gives the guess back on Undo', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		const undo = s.undo(
			{
				kitsu_id: RIGHT.id,
				linked: [{ row: CARD.id, previous: null, ep_no: '1', watched_at: null }]
			},
			GUESS.id,
			CARD.id
		);
		expect(s.linked).toBeNull();
		expect(s.slip).toBe('holding');
		await undo;
		expect(s.slip).toBe('guess');
		expect(f.refreshed).toBe(2);
	});

	it('picking the guess in the picker keeps it, as "That\'s right" does', async () => {
		const f = fake();
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: GUESS, current: true });
		await s.confirm();
		expect(s.slip).toBe('none');
		expect(f.notices.at(-1)!.message).toBe(
			m.play_link_kept_toast({ title: GUESS.canonical_title })
		);
	});

	it('leaves the slip as it was when the link fails', async () => {
		const f = fake();
		f.deps.link = async () => Promise.reject(new Error('down'));
		const s = new InPlaceLinkSession(f.deps);
		await s.arrive(CARD.id, GUESS.id);
		await s.start(2);
		s.choose({ ref: RIGHT, current: false });
		await s.confirm();
		expect(s.slip).toBe('guess');
		expect(s.linked).toBeNull();
		expect(s.error).not.toBeNull();
	});
});
