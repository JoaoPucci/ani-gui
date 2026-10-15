import { afterEach, describe, expect, it } from 'vitest';
import {
	failedHeroImages,
	naturalRatio,
	noteHeroImageFailed,
	pickHeroImage,
	type HeroImageInput
} from './framing';
import type { HeroPrefs } from './prefs';

const cover = {
	tiny: 'https://k/c-tiny.jpg',
	small: 'https://k/c-small.jpg',
	large: 'https://k/c-large.jpg',
	original: 'https://k/c-original.jpg'
};
const poster = {
	tiny: 'https://k/p-tiny.jpg',
	small: 'https://k/p-small.jpg',
	medium: 'https://k/p-medium.jpg',
	large: 'https://k/p-large.jpg',
	original: 'https://k/p-original.jpg'
};
const banner = 'https://a/banner.jpg';

function show(over: Partial<HeroImageInput> = {}): HeroImageInput {
	return { anilist_banner: banner, cover_image: cover, poster_image: poster, ...over };
}
const prefs = (
	framing: HeroPrefs['framing'],
	source: HeroPrefs['source'],
	controls = false
): HeroPrefs => ({ framing, source, controls });

describe('pickHeroImage', () => {
	it('keeps each surface’s existing cover order under top and focus', () => {
		for (const f of ['top', 'focus'] as const) {
			expect(pickHeroImage(show(), prefs(f, 'kitsu'), 'home')).toEqual({
				url: cover.large,
				isCover: true,
				origin: 'kitsu'
			});
			const noLarge = show({ cover_image: { ...cover, large: null } });
			expect(pickHeroImage(noLarge, prefs(f, 'kitsu'), 'home').url).toBe(cover.original);
			expect(pickHeroImage(noLarge, prefs(f, 'kitsu'), 'detail').url).toBe(cover.small);
		}
	});
	it('prefers the whole upload when filling the height', () => {
		expect(pickHeroImage(show(), prefs('pillar', 'kitsu'), 'home').url).toBe(cover.original);
		expect(pickHeroImage(show(), prefs('pillar', 'anilist'), 'detail').url).toBe(banner);
	});
	it('prefers the whole upload when never cropping', () => {
		expect(pickHeroImage(show(), prefs('contain', 'kitsu'), 'detail').url).toBe(cover.original);
		const noOriginal = show({ cover_image: { ...cover, original: null } });
		expect(pickHeroImage(noOriginal, prefs('contain', 'kitsu'), 'home').url).toBe(cover.large);
	});
	it('takes the AniList banner first when the source asks for it', () => {
		expect(pickHeroImage(show(), prefs('focus', 'anilist'), 'detail')).toEqual({
			url: banner,
			isCover: true,
			origin: 'anilist'
		});
	});
	it('falls back to the Kitsu cover when AniList has no banner', () => {
		for (const missing of [null, undefined, '']) {
			const d = show({ anilist_banner: missing });
			expect(pickHeroImage(d, prefs('contain', 'anilist'), 'home')).toEqual({
				url: cover.original,
				isCover: true,
				origin: 'kitsu'
			});
		}
	});
	it('ignores the AniList banner under the Kitsu source', () => {
		expect(pickHeroImage(show(), prefs('top', 'kitsu'), 'home').origin).toBe('kitsu');
	});
	it('falls back to the poster with no cover, in each surface’s order', () => {
		const d = show({ anilist_banner: null, cover_image: null });
		expect(pickHeroImage(d, prefs('focus', 'anilist'), 'home')).toEqual({
			url: poster.large,
			isCover: false,
			origin: 'poster'
		});
		const noLarge = show({
			anilist_banner: null,
			cover_image: null,
			poster_image: { ...poster, large: null }
		});
		expect(pickHeroImage(noLarge, prefs('top', 'kitsu'), 'home').url).toBe(poster.original);
		expect(pickHeroImage(noLarge, prefs('top', 'kitsu'), 'detail').url).toBe(poster.medium);
	});
	it('treats an empty cover bag as no cover', () => {
		const d = show({ anilist_banner: null, cover_image: {} });
		expect(pickHeroImage(d, prefs('contain', 'kitsu'), 'detail').origin).toBe('poster');
	});
	it('has nothing when there is no image at all', () => {
		const d = { cover_image: null, poster_image: null };
		expect(pickHeroImage(d, prefs('focus', 'anilist'), 'detail')).toEqual({
			url: null,
			isCover: false,
			origin: null
		});
	});
});

describe('pickHeroImage — after a failed load', () => {
	const failed = (...urls: string[]) => new Set(urls);

	it('passes over a banner that failed for the cover', () => {
		expect(pickHeroImage(show(), prefs('pillar', 'anilist'), 'home', failed(banner))).toEqual({
			url: cover.original,
			isCover: true,
			origin: 'kitsu'
		});
	});
	it('passes over a cover slot that failed for the next in that framing’s order', () => {
		expect(
			pickHeroImage(show(), prefs('pillar', 'anilist'), 'home', failed(banner, cover.original)).url
		).toBe(cover.large);
		expect(pickHeroImage(show(), prefs('top', 'kitsu'), 'home', failed(cover.large)).url).toBe(
			cover.original
		);
		expect(pickHeroImage(show(), prefs('top', 'kitsu'), 'detail', failed(cover.large)).url).toBe(
			cover.small
		);
	});
	it('falls to the poster once every cover has failed', () => {
		const gone = failed(banner, cover.original, cover.large, cover.small, cover.tiny);
		expect(pickHeroImage(show(), prefs('pillar', 'anilist'), 'home', gone)).toEqual({
			url: poster.large,
			isCover: false,
			origin: 'poster'
		});
		expect(pickHeroImage(show(), prefs('pillar', 'anilist'), 'detail', gone).url).toBe(
			poster.large
		);
	});
	it('passes over a poster slot that failed too, and has nothing once all have', () => {
		const gone = failed(banner, cover.original, cover.large, cover.small, cover.tiny);
		expect(
			pickHeroImage(show(), prefs('pillar', 'anilist'), 'home', new Set([...gone, poster.large]))
				.url
		).toBe(poster.original);
		const all = new Set([...gone, ...Object.values(poster)]);
		expect(pickHeroImage(show(), prefs('pillar', 'anilist'), 'home', all)).toEqual({
			url: null,
			isCover: false,
			origin: null
		});
	});
	it('does not skip a cover that is the banner’s URL unless that URL failed', () => {
		const d = show({ cover_image: { large: banner, original: banner } });
		expect(pickHeroImage(d, prefs('top', 'anilist'), 'home', failed()).url).toBe(banner);
		expect(pickHeroImage(d, prefs('top', 'anilist'), 'home', failed(banner)).origin).toBe('poster');
	});
});

describe('failedHeroImages', () => {
	afterEach(() => failedHeroImages.clear());
	it('remembers what noteHeroImageFailed was told', () => {
		expect(failedHeroImages.has(banner)).toBe(false);
		noteHeroImageFailed(banner);
		expect(failedHeroImages.has(banner)).toBe(true);
	});
});

describe('pickHeroImage — provenance and the pre-banner order', () => {
	it('names a cover that is AniList’s banner as AniList’s', () => {
		// The backend backfills a null Kitsu cover with AniList's banner.
		const d = show({ cover_image: { large: banner, original: banner } });
		expect(pickHeroImage(d, prefs('top', 'kitsu'), 'home')).toEqual({
			url: banner,
			isCover: true,
			origin: 'anilist'
		});
	});
	it('keeps an empty slot as the pages’ `??` chains did', () => {
		const d = show({ cover_image: { ...cover, large: '' } });
		expect(pickHeroImage(d, prefs('top', 'kitsu'), 'home').url).toBe('');
	});
});

describe('naturalRatio', () => {
	it('is width over height', () => {
		expect(naturalRatio(1900, 400)).toBeCloseTo(4.75);
	});
	it('is null for an image that has not decoded', () => {
		expect(naturalRatio(0, 0)).toBeNull();
		expect(naturalRatio(100, 0)).toBeNull();
		expect(naturalRatio(0, 100)).toBeNull();
	});
});
