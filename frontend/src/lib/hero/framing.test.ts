import { describe, expect, it } from 'vitest';
import {
	HERO_EXPERIMENT_DEFAULT,
	HERO_EXPERIMENT_STORAGE_KEY,
	heroExperimentFromQuery,
	loadHeroExperiment,
	naturalRatio,
	parseHeroExperiment,
	pickHeroImage,
	type HeroExperiment,
	type HeroImageInput
} from './framing';

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
const exp = (framing: HeroExperiment['framing'], source: HeroExperiment['source']) => ({
	framing,
	source
});

class MemStorage {
	map = new Map<string, string>();
	getItem(k: string) {
		return this.map.get(k) ?? null;
	}
	setItem(k: string, v: string) {
		this.map.set(k, v);
	}
}

describe('parseHeroExperiment', () => {
	it('reads both fields', () => {
		expect(parseHeroExperiment('{"framing":"contain","source":"anilist"}')).toEqual({
			framing: 'contain',
			source: 'anilist'
		});
	});
	it('falls back per field on unknown values', () => {
		expect(parseHeroExperiment('{"framing":"zoom","source":"anilist"}')).toEqual({
			framing: 'current',
			source: 'anilist'
		});
		expect(parseHeroExperiment('{"framing":"focus","source":7}')).toEqual({
			framing: 'focus',
			source: 'kitsu'
		});
	});
	it('is the default for nothing, junk and non-objects', () => {
		expect(parseHeroExperiment(null)).toEqual(HERO_EXPERIMENT_DEFAULT);
		expect(parseHeroExperiment('{nope')).toEqual(HERO_EXPERIMENT_DEFAULT);
		expect(parseHeroExperiment('null')).toEqual(HERO_EXPERIMENT_DEFAULT);
		expect(parseHeroExperiment('"focus"')).toEqual(HERO_EXPERIMENT_DEFAULT);
	});
});

describe('heroExperimentFromQuery', () => {
	const base = exp('focus', 'anilist');
	it('is null when the query names neither', () => {
		expect(heroExperimentFromQuery('?page=2', base)).toBeNull();
		expect(heroExperimentFromQuery('', base)).toBeNull();
	});
	it('overrides only the named field', () => {
		expect(heroExperimentFromQuery('?hero=contain', base)).toEqual(exp('contain', 'anilist'));
		expect(heroExperimentFromQuery('?heroSrc=kitsu', base)).toEqual(exp('focus', 'kitsu'));
		expect(heroExperimentFromQuery('?hero=current&heroSrc=kitsu', base)).toEqual(
			exp('current', 'kitsu')
		);
	});
	it('ignores an unknown value for that field', () => {
		expect(heroExperimentFromQuery('?hero=zoom&heroSrc=anilist', base)).toEqual(base);
	});
	it('resets to the default', () => {
		expect(heroExperimentFromQuery('?hero=reset', base)).toEqual(HERO_EXPERIMENT_DEFAULT);
	});
});

describe('loadHeroExperiment', () => {
	it('is the default with no storage and no query', () => {
		expect(loadHeroExperiment('', null)).toEqual(HERO_EXPERIMENT_DEFAULT);
	});
	it('reads the stored experiment', () => {
		const s = new MemStorage();
		s.setItem(HERO_EXPERIMENT_STORAGE_KEY, JSON.stringify(exp('focus', 'anilist')));
		expect(loadHeroExperiment('', s)).toEqual(exp('focus', 'anilist'));
	});
	it('applies the query over the stored one and persists it', () => {
		const s = new MemStorage();
		s.setItem(HERO_EXPERIMENT_STORAGE_KEY, JSON.stringify(exp('focus', 'anilist')));
		expect(loadHeroExperiment('?hero=contain', s)).toEqual(exp('contain', 'anilist'));
		expect(JSON.parse(s.getItem(HERO_EXPERIMENT_STORAGE_KEY) ?? 'null')).toEqual(
			exp('contain', 'anilist')
		);
	});
	it('still answers when storage throws', () => {
		const broken = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		};
		expect(loadHeroExperiment('?hero=focus', broken)).toEqual(exp('focus', 'kitsu'));
		expect(loadHeroExperiment('', broken)).toEqual(HERO_EXPERIMENT_DEFAULT);
	});
});

describe('pickHeroImage', () => {
	it('keeps each surface’s existing cover order under current and focus', () => {
		for (const f of ['current', 'focus'] as const) {
			expect(pickHeroImage(show(), exp(f, 'kitsu'), 'home')).toEqual({
				url: cover.large,
				isCover: true,
				origin: 'kitsu'
			});
			const noLarge = show({ cover_image: { ...cover, large: null } });
			expect(pickHeroImage(noLarge, exp(f, 'kitsu'), 'home').url).toBe(cover.original);
			expect(pickHeroImage(noLarge, exp(f, 'kitsu'), 'detail').url).toBe(cover.small);
		}
	});
	it('prefers the whole upload when never cropping', () => {
		expect(pickHeroImage(show(), exp('contain', 'kitsu'), 'detail').url).toBe(cover.original);
		const noOriginal = show({ cover_image: { ...cover, original: null } });
		expect(pickHeroImage(noOriginal, exp('contain', 'kitsu'), 'home').url).toBe(cover.large);
	});
	it('takes the AniList banner first when the source asks for it', () => {
		expect(pickHeroImage(show(), exp('focus', 'anilist'), 'detail')).toEqual({
			url: banner,
			isCover: true,
			origin: 'anilist'
		});
	});
	it('falls back to the Kitsu cover when AniList has no banner', () => {
		for (const missing of [null, undefined, '']) {
			const d = show({ anilist_banner: missing });
			expect(pickHeroImage(d, exp('contain', 'anilist'), 'home')).toEqual({
				url: cover.original,
				isCover: true,
				origin: 'kitsu'
			});
		}
	});
	it('ignores the AniList banner under the Kitsu source', () => {
		expect(pickHeroImage(show(), exp('current', 'kitsu'), 'home').origin).toBe('kitsu');
	});
	it('falls back to the poster with no cover, in each surface’s order', () => {
		const d = show({ anilist_banner: null, cover_image: null });
		expect(pickHeroImage(d, exp('focus', 'anilist'), 'home')).toEqual({
			url: poster.large,
			isCover: false,
			origin: 'poster'
		});
		const noLarge = show({
			anilist_banner: null,
			cover_image: null,
			poster_image: { ...poster, large: null }
		});
		expect(pickHeroImage(noLarge, exp('current', 'kitsu'), 'home').url).toBe(poster.original);
		expect(pickHeroImage(noLarge, exp('current', 'kitsu'), 'detail').url).toBe(poster.medium);
	});
	it('treats an empty cover bag as no cover', () => {
		const d = show({ anilist_banner: null, cover_image: {} });
		expect(pickHeroImage(d, exp('contain', 'kitsu'), 'detail').origin).toBe('poster');
	});
	it('has nothing when there is no image at all', () => {
		const d = { cover_image: null, poster_image: null };
		expect(pickHeroImage(d, exp('focus', 'anilist'), 'detail')).toEqual({
			url: null,
			isCover: false,
			origin: null
		});
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
