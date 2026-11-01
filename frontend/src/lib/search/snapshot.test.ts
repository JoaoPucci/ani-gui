import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef, SearchOrder } from '$lib/api';
import {
	FRESH_FOR_MS,
	MAX_SAVED_HITS,
	canReuse,
	captureSearch,
	pickAnchor,
	readSnapshot,
	resumeStep,
	type CaptureInput,
	type CardPosition,
	type SearchCheckpoint,
	type SearchSnapshot
} from './snapshot';

function ref(id: string, overrides: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Title ${id}`,
		titles: { en_jp: `Romaji ${id}` },
		abbreviated_titles: [`Alias ${id}`],
		slug: `title-${id}`,
		synopsis: 'A long synopsis the grid never shows.',
		start_date: '2019-01-01',
		end_date: '2019-03-30',
		episode_count: 12,
		average_rating: 81.2,
		subtype: 'TV',
		status: 'finished',
		age_rating: 'PG',
		popularity_rank: 120,
		poster_image: {
			tiny: null,
			small: 'https://media.kitsu.app/s.jpg',
			medium: 'https://media.kitsu.app/m.jpg',
			large: null,
			original: null
		},
		cover_image: {
			tiny: null,
			small: null,
			large: 'https://media.kitsu.app/c.jpg',
			original: null
		},
		anilist_banner: 'https://anilist.co/b.jpg',
		...overrides
	};
}

function checkpoint(hits: KitsuAnimeRef[], overrides: Partial<SearchCheckpoint> = {}) {
	return {
		mode: 'sub' as const,
		hits,
		seen: hits.map((h) => h.id),
		next: 20,
		emptyStreak: 0,
		pages: 1,
		phase: 'idle' as const,
		...overrides
	};
}

function input(overrides: Partial<CaptureInput> = {}): CaptureInput {
	return {
		q: 'frieren',
		order: 'year',
		subtypes: ['TV'],
		checkpoint: checkpoint([ref('1'), ref('2')]),
		cards: [],
		now: 1_000_000,
		...overrides
	};
}

/** What the grid needs of a result, and all a saved one keeps. */
const shown = (h: KitsuAnimeRef) => ({
	id: h.id,
	canonical_title: h.canonical_title,
	slug: h.slug,
	start_date: h.start_date,
	episode_count: h.episode_count,
	average_rating: h.average_rating,
	subtype: h.subtype,
	poster_image: h.poster_image
});

describe('captureSearch', () => {
	it('keeps the query, order, type filters, time and results it was given', () => {
		const snap = captureSearch(input());
		expect(snap).toMatchObject({
			v: 1,
			q: 'frieren',
			order: 'year',
			subtypes: ['TV'],
			capturedAt: 1_000_000,
			pages: 1
		});
		expect(snap.results).toMatchObject({
			mode: 'sub',
			seen: ['1', '2'],
			next: 20,
			emptyStreak: 0,
			pages: 1,
			phase: 'idle'
		});
		expect(snap.results!.hits.map((h) => h.id)).toEqual(['1', '2']);
	});

	it('keeps of each result only what the grid shows', () => {
		const [hit] = captureSearch(input({ checkpoint: checkpoint([ref('7')]) })).results!.hits;
		expect(shown(hit)).toEqual(shown(ref('7')));
		expect(hit.synopsis).toBeNull();
		expect(hit.cover_image).toBeNull();
		expect(hit.titles).toBeUndefined();
		expect(hit.abbreviated_titles).toBeUndefined();
		expect(hit.anilist_banner).toBeUndefined();
	});

	it('keeps where the reader was, but not the results, past the most it carries', () => {
		const hits = Array.from({ length: MAX_SAVED_HITS + 1 }, (_, i) => ref(String(i)));
		const snap = captureSearch(
			input({
				checkpoint: checkpoint(hits, { pages: 21 }),
				cards: [{ id: '300', top: -40, bottom: 260 }]
			})
		);
		expect(snap.results).toBeNull();
		expect(snap.pages).toBe(21);
		expect(snap.anchor).toEqual({ id: '300', top: -40 });
	});

	it('keeps no results before a first page has settled', () => {
		const snap = captureSearch(input({ checkpoint: null }));
		expect(snap.results).toBeNull();
		expect(snap.pages).toBe(0);
	});

	it('copies the filters rather than holding the live set', () => {
		const subtypes = ['TV'];
		const snap = captureSearch(input({ subtypes }));
		subtypes.push('movie');
		expect(snap.subtypes).toEqual(['TV']);
	});

	it('anchors on the card at the top of the screen', () => {
		const cards = [
			{ id: '1', top: -600, bottom: -300 },
			{ id: '2', top: -120, bottom: 180 },
			{ id: '3', top: 200, bottom: 500 }
		];
		expect(captureSearch(input({ cards })).anchor).toEqual({ id: '2', top: -120 });
	});
});

describe('pickAnchor', () => {
	it('takes the first card still on screen', () => {
		expect(
			pickAnchor([
				{ id: 'a', top: -400, bottom: -100 },
				{ id: 'b', top: -100, bottom: 0 },
				{ id: 'c', top: 0, bottom: 300 }
			])
		).toEqual({ id: 'c', top: 0 });
	});

	it('has nothing to anchor on with no card on screen', () => {
		expect(pickAnchor([])).toBeNull();
		expect(pickAnchor([{ id: 'a', top: -400, bottom: -100 }])).toBeNull();
	});

	it('answers a card on screen with every card before it scrolled past', () => {
		const card = fc.record({
			id: fc.string({ minLength: 1 }),
			top: fc.integer({ min: -5000, max: 5000 }),
			height: fc.integer({ min: 1, max: 600 })
		});
		fc.assert(
			fc.property(fc.array(card, { maxLength: 30 }), (raw) => {
				const cards: CardPosition[] = raw.map((c) => ({
					id: c.id,
					top: c.top,
					bottom: c.top + c.height
				}));
				const anchor = pickAnchor(cards);
				const at = anchor === null ? cards.length : cards.findIndex((c) => c.id === anchor.id);
				for (const before of cards.slice(0, at)) expect(before.bottom).toBeLessThanOrEqual(0);
				if (anchor) {
					expect(cards[at].bottom).toBeGreaterThan(0);
					expect(anchor.top).toBe(cards[at].top);
				}
			})
		);
	});
});

const order = fc.constantFrom<SearchOrder>('relevance', 'year', 'rating');
const hitArb = fc
	.record({
		id: fc.string({ minLength: 1, maxLength: 6 }),
		title: fc.string({ maxLength: 12 }),
		subtype: fc.option(fc.constantFrom('TV', 'movie', 'OVA'), { nil: null }),
		episodes: fc.option(fc.nat(2000), { nil: null }),
		rating: fc.option(fc.integer({ min: 0, max: 100 }), { nil: null }),
		poster: fc.option(fc.webUrl(), { nil: null })
	})
	.map((r) =>
		ref(r.id, {
			canonical_title: r.title,
			subtype: r.subtype,
			episode_count: r.episodes,
			average_rating: r.rating,
			poster_image: r.poster
				? { tiny: null, small: null, medium: r.poster, large: null, original: null }
				: null
		})
	);
const inputArb = fc.record({
	q: fc.string({ maxLength: 20 }),
	order,
	subtypes: fc.uniqueArray(fc.constantFrom('TV', 'movie', 'special', 'OVA', 'ONA', 'music')),
	checkpoint: fc.option(
		fc.record({
			mode: fc.constantFrom<'sub' | 'dub'>('sub', 'dub'),
			hits: fc.array(hitArb, { maxLength: 8 }),
			seen: fc.array(fc.string({ maxLength: 6 }), { maxLength: 10 }),
			next: fc.option(fc.nat(10_000), { nil: null }),
			emptyStreak: fc.nat(3),
			pages: fc.nat(30),
			phase: fc.constantFrom<'idle' | 'end' | 'held'>('idle', 'end', 'held')
		}),
		{ nil: null }
	),
	cards: fc.array(
		fc.record({
			id: fc.string({ minLength: 1, maxLength: 6 }),
			top: fc.integer({ min: -3000, max: 3000 }),
			bottom: fc.integer({ min: -3000, max: 3000 })
		}),
		{ maxLength: 6 }
	),
	now: fc.nat(2_000_000_000_000)
});

describe('readSnapshot', () => {
	it('reads back what was captured, through the JSON SvelteKit stores it as', () => {
		fc.assert(
			fc.property(inputArb, (i) => {
				const snap = captureSearch(i);
				expect(readSnapshot(JSON.parse(JSON.stringify(snap)))).toEqual(snap);
			})
		);
	});

	it('never throws, whatever sessionStorage hands back', () => {
		fc.assert(
			fc.property(fc.jsonValue(), (raw) => {
				const out = readSnapshot(raw);
				expect(out === null || out.v === 1).toBe(true);
			})
		);
	});

	it('refuses a value that is not this version of a snapshot', () => {
		const snap = captureSearch(input()) as unknown as Record<string, unknown>;
		expect(readSnapshot(null)).toBeNull();
		expect(readSnapshot('frieren')).toBeNull();
		expect(readSnapshot({ ...snap, v: 2 })).toBeNull();
		expect(readSnapshot({ ...snap, q: 7 })).toBeNull();
		expect(readSnapshot({ ...snap, order: 'title' })).toBeNull();
		expect(readSnapshot({ ...snap, subtypes: 'TV' })).toBeNull();
		expect(readSnapshot({ ...snap, capturedAt: '0' })).toBeNull();
		expect(readSnapshot({ ...snap, pages: -1 })).toBeNull();
	});

	it('keeps the query and filters when only the results or the anchor are unreadable', () => {
		const snap = captureSearch(input({ cards: [{ id: '1', top: 0, bottom: 300 }] }));
		const raw = JSON.parse(JSON.stringify(snap));
		for (const results of [
			'x',
			{ ...raw.results, mode: 'raw' },
			{ ...raw.results, hits: [{ id: 3 }] },
			{ ...raw.results, seen: [1] },
			{ ...raw.results, next: 'x' },
			{ ...raw.results, phase: 'loading' },
			{ ...raw.results, pages: 1.5 }
		]) {
			const out = readSnapshot({ ...raw, results });
			expect(out?.results).toBeNull();
			expect(out?.q).toBe('frieren');
			expect(out?.subtypes).toEqual(['TV']);
		}
		expect(readSnapshot({ ...raw, anchor: { id: 1, top: 0 } })?.anchor).toBeNull();
		expect(readSnapshot({ ...raw, anchor: { id: '1' } })?.anchor).toBeNull();
	});

	it('takes only the filters it knows the shape of', () => {
		const raw = JSON.parse(JSON.stringify(captureSearch(input())));
		expect(readSnapshot({ ...raw, subtypes: ['TV', 3] })).toBeNull();
	});

	it('reads a result with fields it does not know as the grid needs it', () => {
		const raw = JSON.parse(JSON.stringify(captureSearch(input())));
		raw.results.hits[0] = {
			id: '1',
			canonical_title: 'Kept',
			subtype: 4,
			poster_image: 'nope',
			episode_count: '12'
		};
		const [hit] = readSnapshot(raw)!.results!.hits;
		expect(hit.canonical_title).toBe('Kept');
		expect(hit.subtype).toBeNull();
		expect(hit.poster_image).toBeNull();
		expect(hit.episode_count).toBeNull();
	});
});

describe('canReuse', () => {
	const saved = (over: Partial<SearchSnapshot> = {}) => ({
		...captureSearch(input({ now: 1_000_000 })),
		...over
	});
	const at = { now: 1_000_000 + 60_000, mode: 'sub' as const, reload: false };

	it('reuses results saved a moment ago, for the same audio, on a back press', () => {
		expect(canReuse(saved(), at)).toBe(true);
	});

	it('asks again once the results are an hour old', () => {
		expect(canReuse(saved(), { ...at, now: 1_000_000 + FRESH_FOR_MS - 1 })).toBe(true);
		expect(canReuse(saved(), { ...at, now: 1_000_000 + FRESH_FOR_MS })).toBe(false);
	});

	it('asks again when the clock reads earlier than the save', () => {
		expect(canReuse(saved(), { ...at, now: 999_999 })).toBe(false);
	});

	it('asks again when the audio setting changed since', () => {
		expect(canReuse(saved(), { ...at, mode: 'dub' })).toBe(false);
	});

	it('asks again on a reload', () => {
		expect(canReuse(saved(), { ...at, reload: true })).toBe(false);
	});

	it('asks again when no results were saved', () => {
		expect(canReuse(saved({ results: null }), at)).toBe(false);
	});

	it('never reuses at a later time what it would not reuse at an earlier one', () => {
		fc.assert(
			fc.property(fc.nat(2 * FRESH_FOR_MS), fc.nat(2 * FRESH_FOR_MS), (a, b) => {
				const [early, late] = a <= b ? [a, b] : [b, a];
				const s = saved();
				if (canReuse(s, { ...at, now: s.capturedAt + late })) {
					expect(canReuse(s, { ...at, now: s.capturedAt + early })).toBe(true);
				}
			})
		);
	});
});

describe('resumeStep', () => {
	const target = { id: '21', pages: 2 };
	const hits = (n: number) => Array.from({ length: n }, (_, i) => ref(String(i + 1)));

	it('has arrived once the card is shown', () => {
		for (const phase of ['idle', 'end', 'held', 'loading-more']) {
			expect(resumeStep(target, { shown: hits(25), pages: 2, phase })).toBe('found');
		}
	});

	it('loads another page while the card is missing and pages remain', () => {
		expect(resumeStep(target, { shown: hits(20), pages: 1, phase: 'idle' })).toBe('more');
	});

	it('stops once the pages the reader had are loaded, or the search has no more', () => {
		expect(resumeStep(target, { shown: hits(20), pages: 2, phase: 'idle' })).toBe('gone');
		expect(resumeStep(target, { shown: hits(20), pages: 1, phase: 'end' })).toBe('gone');
		expect(resumeStep(target, { shown: hits(20), pages: 1, phase: 'held' })).toBe('gone');
	});

	it('waits while the runner is already loading the next page by itself', () => {
		expect(resumeStep(target, { shown: hits(0), pages: 1, phase: 'loading-more' })).toBe('wait');
	});

	it('has nothing to aim for without a card', () => {
		expect(resumeStep({ id: null, pages: 2 }, { shown: hits(20), pages: 1, phase: 'idle' })).toBe(
			'gone'
		);
	});

	it('never loads past the pages the reader had', () => {
		fc.assert(
			fc.property(
				fc.nat(10),
				fc.nat(10),
				fc.constantFrom('idle', 'end', 'held', 'loading-more'),
				(pages, have, phase) => {
					const step = resumeStep(
						{ id: 'absent', pages },
						{ shown: hits(have * 3), pages: have, phase }
					);
					if (have >= pages) expect(step).not.toBe('more');
				}
			)
		);
	});
});
