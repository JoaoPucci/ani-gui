import { describe, expect, it } from 'vitest';
import { altTitlesFromKitsu, yearFromKitsuRef, type KitsuAnimeRef } from '$lib/api';
import { handoffArgs } from './handoff-args';

const detail: KitsuAnimeRef = {
	id: '42',
	canonical_title: 'The Show',
	titles: { en: 'The Show', en_jp: 'Za Shou' },
	synopsis: null,
	poster_image: null,
	cover_image: null,
	episode_count: 12,
	subtype: 'TV',
	start_date: '2019-04-07',
	status: 'finished'
} as unknown as KitsuAnimeRef;

describe('handoffArgs', () => {
	it('sends the same request the embedded play sends, the Kitsu id included', () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 3,
			kitsuId: '42',
			config: { mode: 'dub' },
			detail
		});
		expect(args).toEqual({
			title: 'The Show',
			episode: '3',
			mode: 'dub',
			episode_count: 12,
			year: yearFromKitsuRef(detail),
			subtype: 'TV',
			alt_titles: altTitlesFromKitsu(detail),
			kitsu_id: '42'
		});
	});

	it('hands off the version the embedded play would play', () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 3,
			kitsuId: '42',
			config: { mode: 'sub' },
			detail,
			version: 'standard'
		});
		expect(args.version).toBe('standard');
		expect(
			'version' in
				handoffArgs({
					title: 'The Show',
					episode: 3,
					kitsuId: '42',
					config: { mode: 'sub' },
					detail
				})
		).toBe(false);
	});

	it("hands off the show's remembered captions choice, so the player starts where the embedded one would", () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 3,
			kitsuId: '42',
			config: { mode: 'sub' },
			detail,
			caption: 'off'
		});
		expect(args.caption).toBe('off');
		expect(
			'caption' in
				handoffArgs({
					title: 'The Show',
					episode: 3,
					kitsuId: '42',
					config: { mode: 'sub' },
					detail
				})
		).toBe(false);
	});

	it('defaults to sub, names no quality, and sends no id when the page has none', () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 1,
			kitsuId: '',
			config: { mode: 'weird' },
			detail: null
		});
		expect(args.mode).toBe('sub');
		expect('quality' in args).toBe(false);
		expect(args.episode_count).toBeNull();
		expect(args.subtype).toBeNull();
		expect(args.alt_titles).toEqual([]);
		expect(args.kitsu_id).toBeUndefined();
	});

	it("names a guessed card's row and its guess apart from the page's id, as the embedded play does", () => {
		// The backend notes the card beside the row the handoff writes, and
		// records the guess only on the card's own listing.
		const guessed = {
			title: 'The Show',
			episode: 3,
			kitsuId: '',
			guessRow: 'hianime:the-show-2000-5',
			guessId: '42',
			config: { mode: 'sub' },
			detail
		};
		const args = handoffArgs(guessed);
		expect(args.guess_row).toBe('hianime:the-show-2000-5');
		expect(args.guess_id).toBe('42');
		expect(args.kitsu_id).toBeUndefined();
	});

	it("names the card's row of a session no guess opened, as the embedded play does", () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 3,
			kitsuId: '42',
			cardRow: 'hianime:the-show-2000-5',
			config: { mode: 'sub' },
			detail
		});
		expect(args.card_row).toBe('hianime:the-show-2000-5');
		expect(args.kitsu_id).toBe('42');
	});

	it('names no card when it is given none', () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 3,
			kitsuId: '42',
			config: { mode: 'sub' },
			detail
		});
		expect('guess_row' in args).toBe(false);
		expect('guess_id' in args).toBe(false);
		expect('card_row' in args).toBe(false);
	});
});
