import { describe, expect, it } from 'vitest';
import {
	loadMockPrefs,
	MOCK_OPTIONS,
	MOCK_PREFS_DEFAULT,
	MOCK_PREFS_STORAGE_KEY,
	parseMockPrefs,
	saveMockPrefs
} from './prefs';

function memory(initial: string | null = null) {
	const store = new Map<string, string>();
	if (initial !== null) store.set(MOCK_PREFS_STORAGE_KEY, initial);
	return {
		getItem: (k: string) => store.get(k) ?? null,
		setItem: (k: string, v: string) => void store.set(k, v)
	};
}
const throwing = {
	getItem: (): string | null => {
		throw new Error('denied');
	},
	setItem: () => {
		throw new Error('denied');
	}
};

describe('watch-order mockup prefs', () => {
	it('defaults when nothing or garbage is stored', () => {
		expect(parseMockPrefs(null)).toEqual(MOCK_PREFS_DEFAULT);
		expect(parseMockPrefs('{nope')).toEqual(MOCK_PREFS_DEFAULT);
		expect(parseMockPrefs('3')).toEqual(MOCK_PREFS_DEFAULT);
	});

	it('keeps known choices and replaces unknown ones field by field', () => {
		const p = parseMockPrefs(
			JSON.stringify({ enabled: true, franchise: 'shelf', order: 'sideways' })
		);
		expect(p).toMatchObject({
			enabled: true,
			switcher: false,
			franchise: 'shelf',
			order: 'linked'
		});
	});

	it('offers no switch for a choice the maintainer has made', () => {
		// Relation wording, the watching card, the strip under the
		// masthead and short titles were decided in the first two
		// review rounds; the strip's arrangement, its footer and the
		// franchise link's form in the third. The strip and the setting
		// are the real ones now, so none of these is a mockup option,
		// and nor is the Up next card's look: the player shows the real
		// card, as a poster.
		for (const decided of [
			'upNext',
			'wording',
			'resume',
			'placement',
			'titles',
			'strip',
			'setting',
			'cta',
			'stackHover',
			'footer',
			'stackPosters'
		]) {
			expect(Object.keys(MOCK_OPTIONS)).not.toContain(decided);
			expect(Object.keys(parseMockPrefs(null))).not.toContain(decided);
		}
	});

	it('round-trips through storage', () => {
		const s = memory();
		const next = { ...MOCK_PREFS_DEFAULT, enabled: true, franchise: 'shelf' as const };
		saveMockPrefs(s, next);
		expect(loadMockPrefs(s)).toEqual(next);
	});

	it('survives storage that refuses or is absent', () => {
		expect(loadMockPrefs(throwing)).toEqual(MOCK_PREFS_DEFAULT);
		expect(() => saveMockPrefs(throwing, MOCK_PREFS_DEFAULT)).not.toThrow();
		expect(loadMockPrefs(null)).toEqual(MOCK_PREFS_DEFAULT);
		saveMockPrefs(null, MOCK_PREFS_DEFAULT);
	});
});
