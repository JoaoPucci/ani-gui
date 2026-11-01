import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';
import { decideLook, type CardText, type LookInputs } from './card-look';
import { createLookBook, lookKey, type RememberedCard } from './look-book';

const proxy = (u: string | null) => (u ? `proxy:${u}` : null);

function ref(id: string, poster: string | null = `https://img/${id}.jpg`): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		poster_image: poster ? { medium: poster, large: null, small: null, original: null } : null
	} as unknown as KitsuAnimeRef;
}

function ep(thumb: string | null, title: string | null = 'An Episode'): KitsuEpisode {
	return {
		id: 'e',
		canonical_title: title,
		season_number: 1,
		number: 4,
		relative_number: 4,
		length: 24,
		synopsis: null,
		airdate: null,
		thumbnail: thumb ? { original: thumb } : null
	};
}

const text: CardText = { show: 'Show 1', tag: 4, title: 'An Episode', episodeLabel: null };

function inputs(over: Partial<LookInputs> = {}): LookInputs {
	return { match: undefined, episode: undefined, capReached: false, text: null, ...over };
}

describe('lookKey', () => {
	it('names the row and the episode it records', () => {
		expect(lookKey({ id: 'a', ep_no: '3' })).toBe('a|3');
	});
});

describe('createLookBook', () => {
	it('never swaps a card off the poster it settled on at the cap', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1');
		expect(book.look('a', 'a|1', inputs({ match: m })).look).toBeNull();
		const atCap = book.look('a', 'a|1', inputs({ match: m, capReached: true }));
		expect(atCap.look?.image?.src).toBe('proxy:https://img/1.jpg');
		const late = book.look(
			'a',
			'a|1',
			inputs({ match: m, capReached: true, episode: ep('https://t/1.jpg') })
		);
		expect(late.look).toEqual(atCap.look);
		expect(late.instant).toBe(false);
	});

	it('remembers what the details settled on for the next visit, not the cap', () => {
		const remembered = new Map<string, RememberedCard>();
		const book = createLookBook(proxy, remembered);
		const m = ref('1');
		book.look('a', 'a|1', inputs({ match: m, capReached: true }));
		expect(remembered.has('a|1')).toBe(false);
		book.look(
			'a',
			'a|1',
			inputs({ match: m, capReached: true, episode: ep('https://t/1.jpg'), text })
		);
		expect(remembered.get('a|1')).toEqual({
			look: {
				image: { src: 'proxy:https://t/1.jpg', fallback: 'proxy:https://img/1.jpg' },
				matchId: '1'
			},
			text
		});
	});

	it('draws a remembered card in its final look at once, text included', () => {
		const remembered = new Map<string, RememberedCard>();
		const first = createLookBook(proxy, remembered);
		first.look('a', 'a|1', inputs({ match: ref('1'), episode: ep('https://t/1.jpg'), text }));
		const next = createLookBook(proxy, remembered);
		const drawn = next.look('a', 'a|1', inputs());
		expect(drawn.instant).toBe(true);
		expect(drawn.look?.image?.src).toBe('proxy:https://t/1.jpg');
		expect(drawn.text).toEqual(text);
	});

	it('lets live text replace the remembered text once the row is matched', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look(
			'a',
			'a|1',
			inputs({ match: ref('1'), episode: null, text })
		);
		const live: CardText = { show: 'Show 1', tag: 4, title: null, episodeLabel: 4 };
		const drawn = createLookBook(proxy, remembered).look(
			'a',
			'a|1',
			inputs({ match: ref('1'), text: live })
		);
		expect(drawn.text).toEqual(live);
	});

	it('fades in a look recorded while the card sat on this page as a skeleton', () => {
		const remembered = new Map<string, RememberedCard>();
		const book = createLookBook(proxy, remembered);
		expect(book.look('a', 'a|1', inputs()).look).toBeNull();
		const settled = book.look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		expect(settled.look).not.toBeNull();
		expect(settled.instant).toBe(false);
	});

	it('drops a remembered look the live match contradicts, and resolves afresh', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		const book = createLookBook(proxy, remembered);
		expect(book.look('a', 'a|1', inputs()).look?.matchId).toBe('1');
		expect(book.look('a', 'a|1', inputs({ match: ref('2') })).look).toBeNull();
		expect(remembered.has('a|1')).toBe(false);
		const fresh = book.look('a', 'a|1', inputs({ match: ref('2'), episode: null }));
		expect(fresh.look?.matchId).toBe('2');
		expect(fresh.instant).toBe(false);
	});

	it('keeps a remembered look the live match agrees with', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look(
			'a',
			'a|1',
			inputs({ match: ref('1'), episode: ep('https://t/1.jpg') })
		);
		const book = createLookBook(proxy, remembered);
		const before = book.look('a', 'a|1', inputs());
		const after = book.look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		expect(after.look).toEqual(before.look);
		expect(after.instant).toBe(true);
	});

	it('keeps each card to itself', () => {
		const book = createLookBook(proxy, new Map());
		book.look('a', 'a|1', inputs({ match: ref('1'), capReached: true }));
		expect(book.look('b', 'b|1', inputs({ match: ref('2') })).look).toBeNull();
	});
});

// — Properties. A card is driven through any order of the events that
//   reach it; the book is asked for its look after each one.

type Event =
	| { kind: 'match'; value: KitsuAnimeRef | null }
	| { kind: 'episode'; value: KitsuEpisode | null }
	| { kind: 'cap' }
	| { kind: 'render' };

const matchArb = fc.oneof(
	fc.constant(null),
	fc
		.constantFrom('1', '2')
		.chain((id) =>
			fc.option(fc.constant(`https://img/${id}.jpg`), { nil: null }).map((p) => ref(id, p))
		)
);
const episodeArb = fc.option(
	fc.option(fc.constantFrom('https://t/a.jpg', 'https://t/b.jpg'), { nil: null }).map((t) => ep(t)),
	{ nil: null }
);
const eventArb: fc.Arbitrary<Event> = fc.oneof(
	matchArb.map((value) => ({ kind: 'match' as const, value })),
	episodeArb.map((value) => ({ kind: 'episode' as const, value })),
	fc.constant({ kind: 'cap' as const }),
	fc.constant({ kind: 'render' as const })
);

function drive(events: Event[], book = createLookBook(proxy, new Map())) {
	let state = inputs();
	const looks = [book.look('a', 'a|1', state)];
	for (const e of events) {
		if (e.kind === 'match') state = { ...state, match: e.value };
		else if (e.kind === 'episode') state = { ...state, episode: e.value };
		else if (e.kind === 'cap') state = { ...state, capReached: true };
		looks.push(book.look('a', 'a|1', state));
	}
	return { looks, state };
}

describe('card look properties', () => {
	it('a card never changes artwork once it has settled', () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const { looks } = drive(events);
				const first = looks.findIndex((l) => l.look !== null);
				if (first < 0) return;
				for (const l of looks.slice(first)) expect(l.look).toEqual(looks[first].look);
			})
		);
	});

	it('a matched card has settled by the time its wait runs out', () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const { looks, state } = drive(events);
				if (state.match !== undefined && state.capReached) {
					expect(looks[looks.length - 1].look).not.toBeNull();
				}
			})
		);
	});

	it('a remembered card draws its final look at its first render', () => {
		fc.assert(
			fc.property(
				matchArb.filter((m): m is KitsuAnimeRef => m !== null),
				episodeArb,
				(match, episode) => {
					const remembered = new Map<string, RememberedCard>();
					createLookBook(proxy, remembered).look('a', 'a|1', inputs({ match, episode, text }));
					const truth = decideLook(inputs({ match, episode }), proxy);
					const drawn = createLookBook(proxy, remembered).look('a', 'a|1', inputs());
					expect(drawn.look).toEqual(truth);
					expect(drawn.instant).toBe(true);
					expect(drawn.text).toEqual(text);
				}
			)
		);
	});

	it('a remembered card keeps its look through any events that agree with it', () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const remembered = new Map<string, RememberedCard>();
				createLookBook(proxy, remembered).look(
					'a',
					'a|1',
					inputs({ match: ref('1'), episode: null })
				);
				const agreeing = events.filter((e) => e.kind !== 'match' || e.value?.id === '1');
				const { looks } = drive(agreeing, createLookBook(proxy, remembered));
				for (const l of looks) expect(l.look).toEqual(looks[0].look);
			})
		);
	});

	it('the details settle a card on the same look whichever answer came first', () => {
		fc.assert(
			fc.property(
				matchArb.filter((m): m is KitsuAnimeRef => m !== null),
				episodeArb,
				(match, episode) => {
					const matchFirst = drive([
						{ kind: 'match', value: match },
						{ kind: 'episode', value: episode }
					]).looks;
					const episodeFirst = drive([
						{ kind: 'episode', value: episode },
						{ kind: 'match', value: match }
					]).looks;
					expect(matchFirst[2].look).toEqual(episodeFirst[2].look);
				}
			)
		);
	});
});
