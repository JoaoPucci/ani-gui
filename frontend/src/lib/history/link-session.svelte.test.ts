// The show page's link session: the offer it makes on arrival, the
// picker and the link, against fake app dependencies.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { rememberAudio, storedAudio } from '$lib/play/audio-choice';
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
	/** The path the app is on. */
	path: string;
	notices: LinkNotice[];
	pages: [string, string | null][];
	refreshed: number;
	rows: HistoryEntry[];
}

function fake(over: Partial<LinkSessionDeps> = {}): Fake {
	const f = { notices: [], pages: [], refreshed: 0, rows: [CARD] } as unknown as Fake;
	f.path = `/anime/${GUESS}`;
	f.deps = {
		history: async () => f.rows,
		resolve: async (e) =>
			e.kitsu_id
				? { match: ref(e.kitsu_id), trusted: true }
				: { match: ref(GUESS), trusted: false },
		search: async (q) => (q === 'gintama' ? [RIGHT] : [ref(GUESS)]),
		link: async (_card, rows, kitsuId) => {
			f.rows = f.rows.map((r) => (rows.includes(r.id) ? { ...r, kitsu_id: kitsuId } : r));
			return {
				kitsu_id: kitsuId,
				linked: rows.map((row) => ({ row, previous: null, ep_no: '3', watched_at: null }))
			};
		},
		group: async (_card, rows) => rows,
		notify: (n) => f.notices.push(n),
		showPage: (id, row) => {
			f.pages.push([id, row]);
			f.path = `/anime/${id}`;
		},
		here: () => f.path,
		refresh: () => f.refreshed++,
		...over
	};
	return f;
}

describe('the episode the picker names', () => {
	it('is the one the surface it opened from shows for the card', async () => {
		const s = new LinkSession(fake().deps);
		await s.arrive(CARD.id, GUESS);
		await s.start(7);
		expect(s.episode).toBe('7');
	});

	it('is the episode a Continue click plays when the surface names none', async () => {
		const s = new LinkSession(fake().deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		expect(s.episode).toBe('4');
	});

	it('is what the episode warning measures against the chosen show', async () => {
		const s = new LinkSession(fake().deps);
		await s.arrive(CARD.id, GUESS);
		await s.start(52);
		s.choose({ ref: RIGHT, current: false });
		expect(s.warnings).toContain('episode');
		await s.start(51);
		s.choose({ ref: RIGHT, current: false });
		expect(s.warnings).not.toContain('episode');
	});
});

describe('LinkSession', () => {
	it('offers the link on the page a guessed card opened, and only there', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		expect(s.offer?.entry.id).toBe(CARD.id);
		// The episode a Continue click plays next, as the card shows it:
		// the row is at episode 3.
		expect(s.episode).toBe('4');
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

	it('links to another show and goes there', async () => {
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
	});

	it("offers to change again a row the user named a show for, recording the page's show", async () => {
		const f = fake();
		f.rows = [{ ...CARD, kitsu_id: RIGHT.id, named: true }];
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, RIGHT.id);
		expect(s.offer?.entry.id).toBe(CARD.id);
		expect(s.offer?.current.id).toBe(RIGHT.id);
		expect(s.offer?.saved).toBe(RIGHT.id);
		// One the app recorded from a play, with no naming, is not offered.
		f.rows = [{ ...CARD, kitsu_id: RIGHT.id }];
		await s.arrive(CARD.id, RIGHT.id);
		expect(s.offer).toBeNull();
	});

	it('says the show was saved in a plain notice, with no Undo', async () => {
		// Maintainer's decision: the answer is "is this my show or not",
		// recorded once given; the ⋯ "Not this show?" changes it again.
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		await s.search('gintama');
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(f.notices).toEqual([
			{ kind: 'success', message: m.detail_link_done_toast({ title: 'Gintama' }) }
		]);
	});

	it('linking to the page’s own show refreshes the page in place', async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(f.refreshed).toBe(1);
		// Opened again without the card's row, so the line's room goes.
		expect(f.pages).toEqual([[GUESS, null]]);
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

	it('settles an arrival with no offer, so the page can let the line go', async () => {
		const f = fake();
		f.rows = [{ ...CARD, kitsu_id: GUESS }];
		const s = new LinkSession(f.deps);
		const arriving = s.arrive(CARD.id, GUESS);
		expect(s.settled).toBe(false);
		await arriving;
		expect(s.settled).toBe(true);
		expect(s.offer).toBeNull();
	});

	it('works out the card’s rows when the picker opens, and links those', async () => {
		const HIDDEN: HistoryEntry = {
			id: 'gintama-season-4-77',
			title: 'Gintama Season 4',
			ep_no: '1'
		};
		const link = vi.fn(fake().deps.link);
		const f = fake({ link });
		f.rows = [CARD, HIDDEN];
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		await vi.waitFor(() => expect(s.group).toEqual([CARD.id, HIDDEN.id]));
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(link).toHaveBeenCalledWith(CARD.id, [CARD.id, HIDDEN.id], GUESS);
	});

	it('keeps the latest search’s answer, whatever lands last', async () => {
		const gates: Record<string, () => void> = {};
		const f = fake({
			search: (q) =>
				new Promise((resolve) => {
					gates[q] = () => resolve([ref(q === 'second' ? '2' : q === 'first' ? '1' : GUESS)]);
				})
		});
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		const opening = s.start();
		const first = s.search('first');
		const second = s.search('second');
		gates['second']();
		await second;
		gates['first']();
		await first;
		gates['Gintama Season 4']();
		gates['gintama season 4']?.();
		await opening;
		expect(s.candidates?.map((c) => c.ref.id)).toEqual(['2']);
	});

	it('takes the card’s row alone when the history cannot be read on opening', async () => {
		const link = vi.fn(fake().deps.link);
		const f = fake({ link });
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		const history = f.deps.history;
		f.deps.history = async () => Promise.reject(new Error('down'));
		await s.start();
		await vi.waitFor(() => expect(s.group).toEqual([CARD.id]));
		f.deps.history = history;
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(link).toHaveBeenCalledWith(CARD.id, [CARD.id], GUESS);
	});

	it('names the card on a link even when no row of its listing is left', async () => {
		const link = vi.fn(fake().deps.link);
		const f = fake({ link, group: async () => [] });
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		await vi.waitFor(() => expect(s.group).toEqual([]));
		s.choose(s.candidates![0]);
		await s.confirm();
		expect(link).toHaveBeenCalledWith(CARD.id, [], GUESS);
	});

	it('keeps the latest opening’s rows, whatever lands last', async () => {
		const HIDDEN: HistoryEntry = {
			id: 'gintama-season-4-77',
			title: 'Gintama Season 4',
			ep_no: '1'
		};
		const f = fake();
		f.rows = [CARD, HIDDEN];
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		// The first opening's read of the history is slow, and by the time
		// it lands it no longer holds the hidden row.
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		const history = f.deps.history;
		let slowNext = true;
		f.deps.history = async () => {
			if (slowNext) {
				slowNext = false;
				await held;
				return [CARD];
			}
			return history();
		};
		const firstOpen = s.start();
		s.cancel();
		await s.start();
		await vi.waitFor(() => expect(s.group).toEqual([CARD.id, HIDDEN.id]));
		release();
		await firstOpen;
		await new Promise((r) => setTimeout(r, 0));
		expect(s.group).toEqual([CARD.id, HIDDEN.id]);
	});

	it('leaves the user where they went while the link was out, and keeps the link', async () => {
		for (const pick of ['gintama', null] as const) {
			let release!: () => void;
			const held = new Promise<void>((r) => (release = r));
			const f = fake();
			const link = f.deps.link;
			f.deps.link = async (card, rows, kitsuId) => {
				await held;
				return link(card, rows, kitsuId);
			};
			const s = new LinkSession(f.deps);
			await s.arrive(CARD.id, GUESS);
			await s.start();
			// Another show, or the page's own, which would refresh it.
			if (pick) await s.search(pick);
			s.choose(s.candidates![0]);
			const confirming = s.confirm();
			f.path = '/';
			release();
			expect(await confirming).toBe(true);
			expect(f.rows[0].kitsu_id).toBe(pick ? RIGHT.id : GUESS);
			expect(f.pages).toEqual([]);
			expect(f.refreshed).toBe(0);
			// The saved link still says so.
			expect(f.notices[0].message).toBe(
				m.detail_link_done_toast({ title: pick ? 'Gintama' : s.chosen!.ref.canonical_title })
			);
		}
	});

	it('leaves a later arrival on the same page as it is when an earlier link lands', async () => {
		// The page component stays across a move to the same show from
		// another card: the earlier link is saved, but neither closes the
		// new arrival's offer nor takes the user away from it.
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		const other: HistoryEntry = { ...CARD, id: 'hianime:other-1' };
		const f = fake();
		f.rows = [CARD, other];
		const link = f.deps.link;
		f.deps.link = async (card, rows, kitsuId) => {
			await held;
			return link(card, rows, kitsuId);
		};
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		await s.search('gintama');
		s.choose(s.candidates![0]);
		const confirming = s.confirm();
		await s.arrive(other.id, GUESS);
		release();
		expect(await confirming).toBe(true);
		expect(f.rows[0].kitsu_id).toBe(RIGHT.id);
		expect(s.offer?.entry.id).toBe(other.id);
		expect(f.pages).toEqual([]);
		expect(f.notices[0].message).toBe(m.detail_link_done_toast({ title: 'Gintama' }));
		// Still on the page, whose history the link changed: it reads it
		// again.
		expect(f.refreshed).toBe(1);
	});

	it('says a link that failed after a later arrival in a notice, not in the new picker', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		const other: HistoryEntry = { ...CARD, id: 'hianime:other-1' };
		const f = fake();
		f.rows = [CARD, other];
		f.deps.link = async () => {
			await held;
			throw new Error('down');
		};
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose(s.candidates![0]);
		const confirming = s.confirm();
		await s.arrive(other.id, GUESS);
		await s.start();
		release();
		expect(await confirming).toBe(false);
		expect(s.error).toBeNull();
		expect(f.notices).toEqual([{ kind: 'error', message: m.detail_link_failed() }]);
	});
});

// The viewer corrects the show: a picked show with no Sub or Dub of its
// own takes the one the viewer was watching in, so it plays in it.
describe('the audio a correction carries', () => {
	const RIGHT_ID = RIGHT.id;
	let store: Map<string, string>;
	let refuse = false;
	beforeEach(() => {
		store = new Map();
		refuse = false;
		(globalThis as { localStorage?: unknown }).localStorage = {
			getItem: (k: string) => store.get(k) ?? null,
			setItem: (k: string, v: string) => {
				if (refuse) throw new Error('quota');
				store.set(k, v);
			}
		};
	});
	afterEach(() => {
		delete (globalThis as { localStorage?: unknown }).localStorage;
	});

	async function pick(audio: 'sub' | 'dub' | null, link?: LinkSessionDeps['link']) {
		const f = fake();
		const asked: string[] = [];
		f.deps.audioInForce = (shown) => {
			asked.push(shown);
			return audio;
		};
		if (link) f.deps.link = link;
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		await s.search('gintama');
		s.choose(s.candidates![0]);
		const saved = await s.confirm();
		return { f, s, asked, saved };
	}

	it('is saved as the picked show’s own when it has none, the shown show’s audio asked', async () => {
		const { asked } = await pick('dub');
		expect(storedAudio(RIGHT_ID)).toBe('dub');
		expect(asked).toEqual([GUESS]);
	});

	it('leaves a picked show’s own choice as it is', async () => {
		rememberAudio(RIGHT_ID, 'sub');
		await pick('dub');
		expect(storedAudio(RIGHT_ID)).toBe('sub');
	});

	it('saves nothing when no audio is known yet', async () => {
		await pick(null);
		expect(storedAudio(RIGHT_ID)).toBeUndefined();
	});

	it('saves nothing for a link that failed', async () => {
		const { saved } = await pick('dub', async () => Promise.reject(new Error('down')));
		expect(saved).toBe(false);
		expect(storedAudio(RIGHT_ID)).toBeUndefined();
	});

	it('saves nothing when the show picked is the one shown: there is nothing to carry', async () => {
		const f = fake();
		f.deps.audioInForce = () => 'dub';
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		await s.start();
		s.choose({ ref: s.offer!.current, current: true });
		expect(await s.confirm()).toBe(true);
		expect(storedAudio(GUESS)).toBeUndefined();
	});

	it('keeps the link when the storage refuses the choice', async () => {
		refuse = true;
		const { saved } = await pick('dub');
		expect(saved).toBe(true);
		expect(storedAudio(RIGHT_ID)).toBeUndefined();
	});
});

// The offer's reads, and the picker's searches, are reads of the page
// the session is on: each holds one of the API lane's six connections
// until it answers ($lib/navigation/visit). They take the signal of the
// arrival that asked for them, which the next arrival, or the page
// going, cancels. The rows a link writes are read without one: a link
// still out when the page goes is saved all the same.
describe('the reads of an arrival', () => {
	/** `f`'s reads, each recording the signal it was asked with. */
	function signalled(f: Fake) {
		const asked: { read: string; signal: AbortSignal | undefined }[] = [];
		const { history, resolve, search } = f.deps;
		f.deps.history = (opts) => {
			asked.push({ read: 'history', signal: opts?.signal });
			return history();
		};
		f.deps.resolve = (e, opts) => {
			asked.push({ read: 'resolve', signal: opts?.signal });
			return resolve(e);
		};
		f.deps.search = (q, opts) => {
			asked.push({ read: 'search', signal: opts?.signal });
			return search(q);
		};
		return asked;
	}

	it("take the arrival's signal, which the next arrival cancels", async () => {
		const f = fake();
		const asked = signalled(f);
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		const first = asked.map((a) => a.signal);
		expect(asked.map((a) => a.read)).toEqual(['history', 'resolve']);
		expect(first.every((sig) => sig instanceof AbortSignal && !sig.aborted)).toBe(true);
		await s.arrive(CARD.id, GUESS);
		expect(first.every((sig) => sig!.aborted)).toBe(true);
		expect(asked.at(-1)!.signal!.aborted).toBe(false);
	});

	it("give the picker's searches the arrival's signal", async () => {
		const f = fake();
		const asked = signalled(f);
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		const arrival = asked[0].signal;
		expect(arrival).toBeInstanceOf(AbortSignal);
		asked.length = 0;
		await s.start();
		await s.search('gintama');
		const searches = asked.filter((a) => a.read === 'search');
		expect(searches.length).toBeGreaterThan(0);
		expect(searches.every((a) => a.signal === arrival)).toBe(true);
	});

	it('are all cancelled when the page goes, and nothing they answer is shown', async () => {
		const f = fake();
		const asked = signalled(f);
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		let answer!: (hits: KitsuAnimeRef[]) => void;
		f.deps.search = (_q, opts) => {
			asked.push({ read: 'search', signal: opts?.signal });
			return new Promise((r) => (answer = r));
		};
		void s.start();
		await vi.waitFor(() => expect(answer).toBeDefined());
		s.endReads();
		// The offer's reads and the picker's searches; the rows a link
		// would write are read without a signal.
		const arrival = asked.filter((a) => a.signal !== undefined);
		expect(arrival.map((a) => a.read)).toEqual(['history', 'resolve', 'search', 'search']);
		expect(arrival.every((a) => a.signal!.aborted)).toBe(true);
		answer([RIGHT]);
		await new Promise((r) => setTimeout(r, 0));
		expect(s.candidates).toBeNull();
	});

	it("read the rows the opened picker's link would write with the arrival's signal, and keep no group of a cancelled read", async () => {
		const f = fake();
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		const signals: (AbortSignal | undefined)[] = [];
		const history = f.deps.history;
		let answer!: () => void;
		f.deps.history = (opts) => {
			signals.push(opts?.signal);
			return new Promise((r) => (answer = () => r(history())));
		};
		void s.start();
		await vi.waitFor(() => expect(signals.length).toBe(1));
		expect(signals[0]).toBeInstanceOf(AbortSignal);
		s.endReads();
		expect(signals[0]!.aborted).toBe(true);
		answer();
		await new Promise((r) => setTimeout(r, 0));
		// Neither the rows nor the card alone, which a failed read leaves.
		expect(s.group).toBeNull();
	});

	it('leave the rows a link writes to be read without one', async () => {
		const f = fake();
		const asked = signalled(f);
		const s = new LinkSession(f.deps);
		await s.arrive(CARD.id, GUESS);
		asked.length = 0;
		s.choose({ ref: RIGHT, current: false });
		expect(await s.confirm()).toBe(true);
		expect(asked.length).toBeGreaterThan(0);
		expect(asked.every((a) => a.signal === undefined)).toBe(true);
	});
});
