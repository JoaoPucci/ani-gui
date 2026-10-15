import { afterEach, describe, expect, it } from 'vitest';
import {
	HERO_PREFS_DEFAULT,
	HERO_PREFS_STORAGE_KEY,
	browserStorage,
	loadHeroPrefs,
	parseHeroPrefs,
	saveHeroPrefs,
	type HeroPrefs
} from './prefs';

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
