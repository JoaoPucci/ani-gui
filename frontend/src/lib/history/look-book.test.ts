import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';
import {
	decideLook,
	offeredEpisode,
	type CardText,
	type LookInputs,
	type SettledLook
} from './card-look';
import { createLookBook, lookKey, type RememberedCard } from './look-book';

const proxy = (u: string | null) => (u ? `proxy:${u}` : null);

function ref(id: string, poster: string | null = `https://img/${id}.jpg`): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		poster_image: poster ? { medium: poster, large: null, small: null, original: null } : null
	} as unknown as KitsuAnimeRef;
}

function ep(thumb: string | null, title: string | null = 'An Episode', n = 4): KitsuEpisode {
	return {
		id: `e${n}`,
		canonical_title: title,
		season_number: 1,
		number: n,
		relative_number: n,
		length: 24,
		synopsis: null,
		airdate: null,
		thumbnail: thumb ? { original: thumb } : null
	};
}

const text: CardText = { show: 'Show 1', tag: 4, title: 'An Episode', episodeLabel: null };

function inputs(over: Partial<LookInputs> = {}): LookInputs {
	// The card offers episode 4, the episode ep() describes.
	return {
		match: undefined,
		episode: undefined,
		offered: 4,
		capReached: false,
		text: null,
		...over
	};
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
				matchId: '1',
				episode: 4
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

	it('keeps the remembered text until the live details for the offered episode land', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look(
			'a',
			'a|1',
			inputs({ match: ref('1'), episode: ep('https://t/1.jpg'), text })
		);
		const book = createLookBook(proxy, remembered);
		book.look('a', 'a|1', inputs());
		// Matched, details not in yet: the live text is a stand-in.
		const standIn: CardText = { show: 'Show 1', tag: 4, title: null, episodeLabel: 4 };
		expect(book.look('a', 'a|1', inputs({ match: ref('1'), text: standIn })).text).toEqual(text);
		// Details for another episode are not this card's either.
		expect(
			book.look('a', 'a|1', inputs({ match: ref('1'), episode: ep(null, 'X', 3), text: standIn }))
				.text
		).toEqual(text);
		const live: CardText = { ...text, title: 'Renamed' };
		expect(
			book.look('a', 'a|1', inputs({ match: ref('1'), episode: ep('https://t/1.jpg'), text: live }))
				.text
		).toEqual(live);
	});

	it('resolves afresh when the episode the card offers moves', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1');
		const first = book.look('a', 'a|1', inputs({ match: m, episode: ep('https://t/4.jpg') }));
		expect(first.look?.image?.src).toBe('proxy:https://t/4.jpg');
		// The cap refines and the card now offers 5; the page still holds
		// episode 4's details until episode 5's land.
		const between = book.look(
			'a',
			'a|1',
			inputs({ match: m, offered: 5, episode: ep('https://t/4.jpg') })
		);
		expect(between.look).toBeNull();
		const next = book.look(
			'a',
			'a|1',
			inputs({ match: m, offered: 5, episode: ep('https://t/5.jpg', 'Five', 5) })
		);
		expect(next.look).toEqual({
			image: { src: 'proxy:https://t/5.jpg', fallback: 'proxy:https://img/1.jpg' },
			matchId: '1',
			episode: 5
		});
	});

	it('drops a remembered look made for another episode once the match shows which one is offered', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look(
			'a',
			'a|3',
			inputs({ match: ref('1'), offered: 3, episode: ep('https://t/3.jpg', 'Three', 3), text })
		);
		const book = createLookBook(proxy, remembered);
		expect(book.look('a', 'a|3', inputs()).look?.episode).toBe(3);
		// Episode 3 was finished since: the card offers 4 now.
		expect(book.look('a', 'a|3', inputs({ match: ref('1'), offered: 4 })).look).toBeNull();
		expect(
			book.look('a', 'a|3', inputs({ match: ref('1'), offered: 4, episode: ep('https://t/4.jpg') }))
				.look?.image?.src
		).toBe('proxy:https://t/4.jpg');
	});

	it('does not settle a card that moved on to another episode on the last one\'s "nothing"', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1');
		// Episode 4's lookup found nothing; the card now offers 5.
		const stale = book.look(
			'a',
			'a|1',
			inputs({ match: m, offered: 5, episode: null, episodeFor: 4 })
		);
		expect(stale.look).toBeNull();
		const own = book.look(
			'a',
			'a|1',
			inputs({ match: m, offered: 5, episode: ep('https://t/5.jpg', 'Five', 5), episodeFor: 5 })
		);
		expect(own.look?.image?.src).toBe('proxy:https://t/5.jpg');
	});

	it('gives way when the match changes under a look pinned on this page', () => {
		const book = createLookBook(proxy, new Map());
		book.look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		expect(book.look('a', 'a|1', inputs({ match: ref('2'), episode: null })).look?.matchId).toBe(
			'2'
		);
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

	it('drops a remembered look when the row now matches nothing', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		const book = createLookBook(proxy, remembered);
		expect(book.look('a', 'a|1', inputs()).look?.matchId).toBe('1');
		const now = book.look('a', 'a|1', inputs({ match: null }));
		expect(now.look).toEqual({ image: null, matchId: null, episode: null });
		expect(now.instant).toBe(false);
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

	it('takes a still that lands later for the episode it settled on without one', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1');
		const settled = book.look('a', 'a|1', inputs({ match: m, episode: ep(null) }));
		expect(settled.look?.image?.src).toBe('proxy:https://img/1.jpg');
		// A still AniList's pacing held back lands on the same episode.
		const landed = book.look('a', 'a|1', inputs({ match: m, episode: ep('https://t/1.jpg') }));
		expect(landed.look).toEqual({
			image: { src: 'proxy:https://t/1.jpg', fallback: 'proxy:https://img/1.jpg' },
			matchId: '1',
			episode: 4
		});
		expect(landed.landedOver).toEqual(settled.look);
		expect(landed.instant).toBe(false);
	});

	it('takes a landed still after settling at the cap, once the details had none', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1');
		book.look('a', 'a|1', inputs({ match: m, capReached: true }));
		const details = book.look(
			'a',
			'a|1',
			inputs({ match: m, capReached: true, episode: ep(null) })
		);
		expect(details.look?.image?.src).toBe('proxy:https://img/1.jpg');
		const landed = book.look(
			'a',
			'a|1',
			inputs({ match: m, capReached: true, episode: ep('https://t/1.jpg') })
		);
		expect(landed.look?.image?.src).toBe('proxy:https://t/1.jpg');
	});

	it('takes a landed still over the two-letter placeholder, which stays under it', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1', null);
		const settled = book.look('a', 'a|1', inputs({ match: m, episode: ep(null) }));
		expect(settled.look?.image).toBeNull();
		expect(settled.landedOver).toBeNull();
		const landed = book.look('a', 'a|1', inputs({ match: m, episode: ep('https://t/1.jpg') }));
		expect(landed.look?.image).toEqual({ src: 'proxy:https://t/1.jpg', fallback: null });
		expect(landed.landedOver?.image).toBeNull();
	});

	it('keeps the still it shows when another one turns up, and never drops it', () => {
		const book = createLookBook(proxy, new Map());
		const m = ref('1');
		const shown = book.look('a', 'a|1', inputs({ match: m, episode: ep('https://t/1.jpg') }));
		const other = book.look('a', 'a|1', inputs({ match: m, episode: ep('https://t/2.jpg') }));
		expect(other.look).toEqual(shown.look);
		const gone = book.look('a', 'a|1', inputs({ match: m, episode: ep(null) }));
		expect(gone.look).toEqual(shown.look);
	});

	it('takes a still landing over a remembered look only once this visit has seen it without one', () => {
		const remembered = new Map<string, RememberedCard>();
		createLookBook(proxy, remembered).look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		const book = createLookBook(proxy, remembered);
		const drawn = book.look('a', 'a|1', inputs());
		// The live details already carry a still: that is no landing, the
		// card keeps what it drew.
		expect(
			book.look('a', 'a|1', inputs({ match: ref('1'), episode: ep('https://t/1.jpg') })).look
		).toEqual(drawn.look);
		// Another visit, remembered on the poster again: this time the
		// live details come without a still, and the still lands after.
		const seeded = new Map<string, RememberedCard>();
		createLookBook(proxy, seeded).look('a', 'a|1', inputs({ match: ref('1'), episode: null }));
		const again = createLookBook(proxy, seeded);
		expect(again.look('a', 'a|1', inputs()).look?.image?.src).toBe('proxy:https://img/1.jpg');
		again.look('a', 'a|1', inputs({ match: ref('1'), episode: ep(null) }));
		expect(
			again.look('a', 'a|1', inputs({ match: ref('1'), episode: ep('https://t/1.jpg') })).look
				?.image?.src
		).toBe('proxy:https://t/1.jpg');
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
	| { kind: 'answer'; for: number; value: KitsuEpisode | null }
	| { kind: 'offer'; value: number }
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
/** Details for episode 4 or 5, whichever the card offers or not. */
const anyEpisodeArb = fc.option(
	fc
		.tuple(
			fc.option(fc.constantFrom('https://t/a.jpg', 'https://t/b.jpg'), { nil: null }),
			fc.constantFrom(4, 5)
		)
		.map(([t, n]) => ep(t, 'An Episode', n)),
	{ nil: null }
);
const eventArb: fc.Arbitrary<Event> = fc.oneof(
	matchArb.map((value) => ({ kind: 'match' as const, value })),
	anyEpisodeArb.map((value) => ({ kind: 'episode' as const, value })),
	fc.constantFrom(4, 5).map((value) => ({ kind: 'offer' as const, value })),
	// An answer as the page holds it: the episode it was fetched for,
	// and what was found for it (null: nothing).
	fc
		.tuple(
			fc.constantFrom(4, 5),
			fc.option(fc.constant('https://t/c.jpg'), { nil: null }),
			fc.boolean()
		)
		.map(([n, t, found]) => ({
			kind: 'answer' as const,
			for: n,
			value: found ? ep(t, 'An Episode', n) : null
		})),
	fc.constant({ kind: 'cap' as const }),
	fc.constant({ kind: 'render' as const })
);

function drive(events: Event[], book = createLookBook(proxy, new Map())) {
	let state = inputs();
	const states = [state];
	const looks = [book.look('a', 'a|1', state)];
	for (const e of events) {
		if (e.kind === 'match') state = { ...state, match: e.value };
		else if (e.kind === 'episode') state = { ...state, episode: e.value, episodeFor: undefined };
		else if (e.kind === 'answer') state = { ...state, episode: e.value, episodeFor: e.for };
		else if (e.kind === 'offer') state = { ...state, offered: e.value };
		else if (e.kind === 'cap') state = { ...state, capReached: true };
		states.push(state);
		looks.push(book.look('a', 'a|1', state));
	}
	return { looks, states, state };
}

/** What a card stands for: its show and the episode it offers. */
function identity(s: LookInputs): string {
	return s.match === undefined ? 'unmatched' : `${s.match?.id ?? 'none'}|${s.offered}`;
}

/** A still is an episode's thumbnail, not the show's poster. */
const isStill = (src: string | undefined) => src?.startsWith('proxy:https://t/') ?? false;

/**
 * Whether `after` is `before` with a still that landed for the episode
 * the card offers: the details for that episode had none and now have
 * one, and the card had none on show.
 */
function stillLanded(
	before: SettledLook | null,
	after: SettledLook | null,
	was: LookInputs,
	now: LookInputs
): boolean {
	if (before === null || after === null || isStill(before.image?.src)) return false;
	if (!isStill(after.image?.src)) return false;
	const had = offeredEpisode(was.episode, was.offered, was.episodeFor);
	const has = offeredEpisode(now.episode, now.offered, now.episodeFor);
	return (
		had !== undefined &&
		!had?.thumbnail?.original &&
		after.image?.src === proxy(has?.thumbnail?.original ?? null)
	);
}

describe('card look properties', () => {
	it('a card never changes artwork while it offers the same show and episode, but to take a still that lands', () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const { looks, states } = drive(events);
				for (let i = 1; i < looks.length; i++) {
					if (looks[i - 1].look === null) continue;
					if (identity(states[i - 1]) !== identity(states[i])) continue;
					if (stillLanded(looks[i - 1].look, looks[i].look, states[i - 1], states[i])) continue;
					expect(looks[i].look).toEqual(looks[i - 1].look);
				}
			})
		);
	});

	it('a card takes a still only for the episode it offers, in place of a look without one', () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const { looks, states } = drive(events);
				for (let i = 1; i < looks.length; i++) {
					const before = looks[i - 1].look;
					const after = looks[i].look;
					if (before === null || identity(states[i - 1]) !== identity(states[i])) continue;
					if (JSON.stringify(before) === JSON.stringify(after)) continue;
					expect(stillLanded(before, after, states[i - 1], states[i])).toBe(true);
					expect(looks[i].landedOver).toEqual(before);
				}
			})
		);
	});

	it("a card's artwork always belongs to the show and episode it offers", () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const { looks, states } = drive(events);
				looks.forEach((l, i) => {
					const s = states[i];
					if (l.look === null || s.match === undefined) return;
					expect(l.look.matchId).toBe(s.match?.id ?? null);
					if (s.match !== null) expect(l.look.episode).toBe(s.offered);
				});
			})
		);
	});

	it('a card settles before its wait is over only on an answer for the episode it offers', () => {
		const belongs = (s: LookInputs) =>
			s.episode !== undefined &&
			(s.episodeFor !== undefined
				? s.episodeFor === s.offered
				: s.episode === null || s.episode.number === s.offered);
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 12 }), (events) => {
				const { looks, states } = drive(events);
				let heard = false;
				states.forEach((s, i) => {
					if (i > 0 && identity(states[i - 1]) !== identity(s)) heard = false;
					heard = heard || belongs(s);
					if (s.match && !s.capReached && !heard) expect(looks[i].look).toBeNull();
				});
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
				const agreeing = events.filter(
					(e) =>
						(e.kind !== 'match' || e.value?.id === '1') && (e.kind !== 'offer' || e.value === 4)
				);
				const { looks, states } = drive(agreeing, createLookBook(proxy, remembered));
				for (let i = 1; i < looks.length; i++) {
					if (stillLanded(looks[i - 1].look, looks[i].look, states[i - 1], states[i])) continue;
					expect(looks[i].look).toEqual(looks[i - 1].look);
				}
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
