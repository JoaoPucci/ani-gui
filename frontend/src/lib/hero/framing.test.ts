import { afterEach, describe, expect, it } from 'vitest';
import {
	HERO_PREFS_DEFAULT,
	HERO_PREFS_STORAGE_KEY,
	browserStorage,
	loadHeroPrefs,
	naturalRatio,
	parseHeroPrefs,
	pickHeroImage,
	saveHeroPrefs,
	type HeroImageInput,
	type HeroPrefs
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
const prefs = (
	framing: HeroPrefs['framing'],
	source: HeroPrefs['source'],
	controls = false
): HeroPrefs => ({ framing, source, controls });

class MemStorage {
	map = new Map<string, string>();
	getItem(k: string) {
		return this.map.get(k) ?? null;
	}
	setItem(k: string, v: string) {
		this.map.set(k, v);
	}
}
const broken = {
	getItem: () => {
		throw new Error('denied');
	},
	setItem: () => {
		throw new Error('denied');
	}
};

describe('the defaults', () => {
	it('fill the height with AniList’s banner first, controls hidden', () => {
		expect(HERO_PREFS_DEFAULT).toEqual({ framing: 'pillar', source: 'anilist', controls: false });
	});
	it('are kept under a key of their own, not the experiment’s', () => {
		expect(HERO_PREFS_STORAGE_KEY).toBe('ani-gui:hero-framing');
	});
});

describe('parseHeroPrefs', () => {
	it('reads every field', () => {
		expect(parseHeroPrefs('{"framing":"contain","source":"anilist","controls":true}')).toEqual(
			prefs('contain', 'anilist', true)
		);
	});
	it('falls back per field on unknown values', () => {
		expect(parseHeroPrefs('{"framing":"zoom","source":"anilist"}')).toEqual(
			prefs(HERO_PREFS_DEFAULT.framing, 'anilist')
		);
		expect(parseHeroPrefs('{"framing":"focus","source":7}')).toEqual(
			prefs('focus', HERO_PREFS_DEFAULT.source)
		);
	});
	it('shows the controls only on a stored true', () => {
		for (const v of ['"yes"', '1', 'null', '{}']) {
			expect(parseHeroPrefs(`{"controls":${v}}`).controls).toBe(false);
		}
		expect(parseHeroPrefs('{"controls":true}').controls).toBe(true);
	});
	it('names every framing', () => {
		for (const f of ['top', 'contain', 'pillar', 'focus'] as const) {
			expect(parseHeroPrefs(`{"framing":"${f}"}`).framing).toBe(f);
		}
	});
	it('is the default for nothing, junk and non-objects', () => {
		expect(parseHeroPrefs(null)).toEqual(HERO_PREFS_DEFAULT);
		expect(parseHeroPrefs('{nope')).toEqual(HERO_PREFS_DEFAULT);
		expect(parseHeroPrefs('null')).toEqual(HERO_PREFS_DEFAULT);
		expect(parseHeroPrefs('"focus"')).toEqual(HERO_PREFS_DEFAULT);
	});
});

describe('loadHeroPrefs', () => {
	it('is the default with no storage', () => {
		expect(loadHeroPrefs(null)).toEqual(HERO_PREFS_DEFAULT);
	});
	it('reads the stored preferences', () => {
		const s = new MemStorage();
		s.setItem(HERO_PREFS_STORAGE_KEY, JSON.stringify(prefs('focus', 'anilist', true)));
		expect(loadHeroPrefs(s)).toEqual(prefs('focus', 'anilist', true));
	});
	it('is the default when storage refuses', () => {
		expect(loadHeroPrefs(broken)).toEqual(HERO_PREFS_DEFAULT);
	});
});

describe('saveHeroPrefs', () => {
	it('round-trips through storage', () => {
		const s = new MemStorage();
		saveHeroPrefs(s, prefs('pillar', 'kitsu', true));
		expect(loadHeroPrefs(s)).toEqual(prefs('pillar', 'kitsu', true));
	});
	it('is quiet with no storage and when storage refuses', () => {
		expect(() => saveHeroPrefs(null, HERO_PREFS_DEFAULT)).not.toThrow();
		expect(() => saveHeroPrefs(broken, HERO_PREFS_DEFAULT)).not.toThrow();
	});
});

describe('browserStorage', () => {
	const g = globalThis as { localStorage?: unknown };
	const had = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
	afterEach(() => {
		if (had) Object.defineProperty(globalThis, 'localStorage', had);
		else delete g.localStorage;
	});
	it('is null where the window has none', () => {
		delete g.localStorage;
		expect(browserStorage()).toBeNull();
	});
	it('is the window’s storage when there is one', () => {
		const s = new MemStorage();
		Object.defineProperty(globalThis, 'localStorage', { value: s, configurable: true });
		expect(browserStorage()).toBe(s);
	});
	it('is null when reaching it throws', () => {
		Object.defineProperty(globalThis, 'localStorage', {
			get() {
				throw new Error('denied');
			},
			configurable: true
		});
		expect(browserStorage()).toBeNull();
	});
});

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
