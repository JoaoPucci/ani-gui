import { describe, expect, it } from 'vitest';
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';
import { cardText, decideLook, posterSource, visibleImage, type LookInputs } from './card-look';

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

function inputs(over: Partial<LookInputs> = {}): LookInputs {
	return { match: undefined, episode: undefined, capReached: false, text: null, ...over };
}

describe('decideLook', () => {
	it('is a skeleton while the row is being matched, whatever else is known', () => {
		expect(decideLook(inputs(), proxy)).toBeNull();
		expect(
			decideLook(inputs({ episode: ep('https://t/1.jpg'), capReached: true }), proxy)
		).toBeNull();
	});

	it('settles on the placeholder when the row has no match', () => {
		expect(decideLook(inputs({ match: null }), proxy)).toEqual({ image: null, matchId: null });
	});

	it('is a skeleton while a matched card waits for its episode details', () => {
		expect(decideLook(inputs({ match: ref('1') }), proxy)).toBeNull();
	});

	it('settles on the thumbnail, with the poster to fall back on', () => {
		expect(decideLook(inputs({ match: ref('1'), episode: ep('https://t/1.jpg') }), proxy)).toEqual({
			image: { src: 'proxy:https://t/1.jpg', fallback: 'proxy:https://img/1.jpg' },
			matchId: '1'
		});
	});

	it('settles on the poster when the episode has no thumbnail, or no details at all', () => {
		const poster = { image: { src: 'proxy:https://img/1.jpg', fallback: null }, matchId: '1' };
		expect(decideLook(inputs({ match: ref('1'), episode: ep(null) }), proxy)).toEqual(poster);
		expect(decideLook(inputs({ match: ref('1'), episode: null }), proxy)).toEqual(poster);
	});

	it('settles on the poster once the wait runs out', () => {
		expect(decideLook(inputs({ match: ref('1'), capReached: true }), proxy)).toEqual({
			image: { src: 'proxy:https://img/1.jpg', fallback: null },
			matchId: '1'
		});
	});

	it('settles on the placeholder when neither picture exists', () => {
		expect(decideLook(inputs({ match: ref('1', null), episode: null }), proxy)).toEqual({
			image: null,
			matchId: '1'
		});
		expect(decideLook(inputs({ match: ref('1', null), capReached: true }), proxy)).toEqual({
			image: null,
			matchId: '1'
		});
	});

	it('keeps a thumbnail with no poster behind it', () => {
		expect(
			decideLook(inputs({ match: ref('1', null), episode: ep('https://t/1.jpg') }), proxy)
		).toEqual({ image: { src: 'proxy:https://t/1.jpg', fallback: null }, matchId: '1' });
	});
});

describe('posterSource', () => {
	it('prefers the medium poster and falls through the sizes', () => {
		const sized = (p: Record<string, string | null>) =>
			({
				id: '1',
				poster_image: { medium: null, large: null, small: null, original: null, ...p }
			}) as unknown as KitsuAnimeRef;
		expect(posterSource(sized({ medium: 'm', large: 'l' }))).toBe('m');
		expect(posterSource(sized({ large: 'l', small: 's' }))).toBe('l');
		expect(posterSource(sized({ small: 's', original: 'o' }))).toBe('s');
		expect(posterSource(sized({ original: 'o' }))).toBe('o');
		expect(posterSource(sized({}))).toBeNull();
		expect(posterSource(null)).toBeNull();
		expect(posterSource(undefined)).toBeNull();
	});
});

describe('cardText', () => {
	it("carries the show's name, the episode title and the number on the artwork", () => {
		expect(cardText(ep(null, 'Title'), false, 5, 'Bebop')).toEqual({
			show: 'Bebop',
			tag: 5,
			title: 'Title',
			episodeLabel: null
		});
	});
	it('falls back to an "Episode N" line when the episode has no title', () => {
		expect(cardText(ep(null, null), false, 5, 'Bebop')).toEqual({
			show: 'Bebop',
			tag: 5,
			title: null,
			episodeLabel: 5
		});
		expect(cardText(undefined, false, 5, 'Bebop')).toEqual({
			show: 'Bebop',
			tag: 5,
			title: null,
			episodeLabel: 5
		});
	});
	it('gives a single-video show no number and no episode line', () => {
		expect(cardText(null, true, 1, 'Bebop')).toEqual({
			show: 'Bebop',
			tag: null,
			title: null,
			episodeLabel: null
		});
	});
});

describe('visibleImage', () => {
	const image = { src: 'thumb', fallback: 'poster' };
	it('shows the image, then its fallback, then nothing as each fails', () => {
		expect(visibleImage(image, {})).toBe('thumb');
		expect(visibleImage(image, { thumb: true })).toBe('poster');
		expect(visibleImage(image, { thumb: true, poster: true })).toBeNull();
		expect(visibleImage({ src: 'thumb', fallback: null }, { thumb: true })).toBeNull();
		expect(visibleImage(null, {})).toBeNull();
	});
});
