// The show page's link session: the offer it makes on arrival, the
// picker, the link and the undo, against fake app dependencies.

import { describe, expect, it, vi } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { LinkSession, type LinkNotice, type LinkSessionDeps } from './link-session.svelte';

function ref(id: string, over: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: 'TV',
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null,
		...over
	};
}

const CARD: HistoryEntry = {
	id: 'hianime:gintama-season-4-1381',
	title: 'Gintama Season 4',
	ep_no: '3'
};
const GUESS = '10083';
const RIGHT = ref('9969', { canonical_title: 'Gintama', episode_count: 51 });

interface Fake {
	deps: LinkSessionDeps;
	notices: LinkNotice[];
	pages: [string, string | null][];
	refreshed: number;
	rows: HistoryEntry[];
}

function fake(over: Partial<LinkSessionDeps> = {}): Fake {
	const f = { notices: [], pages: [], refreshed: 0, rows: [CARD] } as unknown as Fake;
	f.deps = {
		history: async () => f.rows,
		resolve: async (e) =>
			e.kitsu_id
				? { match: ref(e.kitsu_id), trusted: true }
				: { match: ref(GUESS), trusted: false },
		search: async (q) => (q === 'gintama' ? [RIGHT] : [ref(GUESS)]),
		link: async (rows, kitsuId) => {
			f.rows = f.rows.map((r) => (rows.includes(r.id) ? { ...r, kitsu_id: kitsuId } : r));
			return {
				kitsu_id: kitsuId,
				linked: rows.map((row) => ({ row, previous: null, ep_no: '3', watched_at: null }))
			};
		},
		unlink: async (_k, rows) => {
			f.rows = f.rows.map((r) =>
				rows.some((l) => l.row === r.id) ? { ...r, kitsu_id: undefined } : r
			);
			return { restored: rows.map((r) => r.row), kept: [] };
		},
		notify: (n) => f.notices.push(n),
		showPage: (id, row) => f.pages.push([id, row]),
		refresh: () => f.refreshed++,
		...over
	};
	return f;
}

describe('LinkSession', () => {
	it('offers the link on the page a guessed card opened, and only there', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		expect(s.offer?.entry.id).toBe(CARD.id);
		expect(s.episode).toBe('3');
		await s.arrive(null, GUESS);
		expect(s.offer).toBeNull();
	});

	it('keeps only the latest arrival’s offer', async () => {
		let release!: () => void;
		const slow = new Promise<void>((r) => (release = r));
		const f = fake();
		const history = f.deps.history;
		let first = true;
		f.deps.history = async () => {
			if (first) {
				first = false;
				await slow;
			}
			return history();
		};
		const s = new LinkSession(f.deps);
		const stale = s.arrive(CARD.id, GUESS);
		await s.arrive(null, '1');
		release();
		await stale;
		expect(s.offer).toBeNull();
	});

	it('opens on the candidates, searches, and warns about the chosen show', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		expect(s.open).toBe(true);
		expect(s.candidates?.[0]).toEqual({ ref: ref(GUESS), current: true });
		await s.search('gintama');
		expect(s.candidates?.map((c) => c.ref.id)).toEqual([RIGHT.id]);
		s.choose(s.candidates![0]);
		expect(s.warnings).toEqual(['season']);
		s.choose(null);
		expect(s.chosen).toBeNull();
		s.cancel();
		expect(s.open).toBe(false);
	});

	it('says a failed search failed', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		f.deps.search = async () => Promise.reject(new Error('down'));
		await s.search('gintama');
		expect(s.candidates).toEqual([]);
		expect(s.failed).toBe(true);
	});

	it('links to another show, goes there, and offers an undo that comes back', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		await s.search('gintama');
		s.choose(s.candidates![0]);
		await s.confirm();

		expect(f.rows[0].kitsu_id).toBe(RIGHT.id);
		expect(s.open).toBe(false);
		expect(s.offer).toBeNull();
		expect(f.pages).toEqual([[RIGHT.id, null]]);
		const done = f.notices[0];
		expect(done.message).toBe(m.detail_link_done_toast({ title: 'Gintama' }));
		expect(done.actionLabel).toBe(m.detail_link_undo());

		done.onAction!();
		await vi.waitFor(() => expect(f.pages.length).toBe(2));
		expect(f.rows[0].kitsu_id).toBeUndefined();
		expect(f.pages[1]).toEqual([GUESS, CARD.id]);
		expect(f.notices[1].message).toBe(m.detail_link_undone_toast());
	});

	it('linking to the page’s own show refreshes the page in place', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(f.refreshed).toBe(1);
		expect(f.pages).toEqual([]);
	});

	it('says when an undo met a row played since', async () => {
		const f = fake({ unlink: async () => ({ restored: [], kept: [CARD.id] }) });
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose(s.candidates![0]);
		await s.confirm();
		f.notices[0].onAction!();
		await vi.waitFor(() => expect(f.notices.length).toBe(2));
		expect(f.notices[1]).toEqual({ kind: 'error', message: m.detail_link_undo_kept_toast() });
	});

	it('keeps the picker open with the reason when Kitsu says the show is gone', async () => {
		const f = fake({
			link: async () => Promise.reject({ kind: 'upstream', status: 404 })
		});
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(s.open).toBe(true);
		expect(s.error).toBe(m.detail_link_failed_gone());
		expect(s.busy).toBe(false);
		f.deps.link = async () => Promise.reject(new Error('network'));
		await s.confirm();
		expect(s.error).toBe(m.detail_link_failed());
	});

	it('does nothing to start or confirm without an offer or a choice', async () => {
		const link = vi.fn();
		const f = fake({ link });
		const s = new LinkSession(f.deps);
		await s.start();
		expect(s.open).toBe(false);
		await s.search('x');
		expect(s.candidates).toBeNull();
		await s.arrive(CARD.id, GUESS);
		await s.confirm();
		expect(link).not.toHaveBeenCalled();
	});

	it('says an undo that could not be made failed, and stays put', async () => {
		const f = fake({ unlink: async () => Promise.reject(new Error('down')) });
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose(s.candidates![0]);
		await s.confirm();
		f.notices[0].onAction!();
		await vi.waitFor(() => expect(f.notices.length).toBe(2));
		expect(f.notices[1]).toEqual({ kind: 'error', message: m.detail_link_failed() });
		expect(f.pages).toEqual([]);
	});
});
