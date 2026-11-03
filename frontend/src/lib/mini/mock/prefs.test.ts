import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	MINI_MOCK_OPTIONS,
	MINI_MOCK_OPTION_KEYS,
	MINI_MOCK_PREFS_DEFAULT,
	MINI_MOCK_PREFS_STORAGE_KEY,
	loadMiniMockPrefs,
	parseMiniMockPrefs,
	saveMiniMockPrefs
} from './prefs';

function memory(initial: Record<string, string> = {}) {
	const map = new Map(Object.entries(initial));
	return {
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		map
	};
}

describe('mini-player mockup prefs', () => {
	it('starts off, with the recommended options', () => {
		expect(parseMiniMockPrefs(null)).toEqual(MINI_MOCK_PREFS_DEFAULT);
		expect(MINI_MOCK_PREFS_DEFAULT.enabled).toBe(false);
		expect(MINI_MOCK_PREFS_DEFAULT.switcher).toBe(false);
	});

	it('reads back what was saved', () => {
		const storage = memory();
		const prefs = {
			...MINI_MOCK_PREFS_DEFAULT,
			enabled: true,
			hintSeen: true,
			layout: 'centre' as const,
			platform: 'windows' as const
		};
		saveMiniMockPrefs(storage, prefs);
		expect(storage.map.has(MINI_MOCK_PREFS_STORAGE_KEY)).toBe(true);
		expect(loadMiniMockPrefs(storage)).toEqual(prefs);
	});

	it('falls back per field on anything it does not know', () => {
		const prefs = parseMiniMockPrefs(
			JSON.stringify({ enabled: 'yes', layout: 'sideways', main: 'placeholder' })
		);
		expect(prefs.enabled).toBe(false);
		expect(prefs.layout).toBe(MINI_MOCK_PREFS_DEFAULT.layout);
		expect(prefs.main).toBe('placeholder');
	});

	it('reads broken text and non-objects as the default', () => {
		expect(parseMiniMockPrefs('{')).toEqual(MINI_MOCK_PREFS_DEFAULT);
		expect(parseMiniMockPrefs('3')).toEqual(MINI_MOCK_PREFS_DEFAULT);
	});

	it('survives storage that throws or is missing', () => {
		const throwing = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('full');
			}
		};
		expect(loadMiniMockPrefs(throwing)).toEqual(MINI_MOCK_PREFS_DEFAULT);
		expect(() => saveMiniMockPrefs(throwing, MINI_MOCK_PREFS_DEFAULT)).not.toThrow();
		expect(loadMiniMockPrefs(null)).toEqual(MINI_MOCK_PREFS_DEFAULT);
		expect(() => saveMiniMockPrefs(null, MINI_MOCK_PREFS_DEFAULT)).not.toThrow();
	});

	it('always yields an allowed value for every option, whatever was stored', () => {
		fc.assert(
			fc.property(fc.dictionary(fc.string(), fc.jsonValue()), (stored) => {
				const prefs = parseMiniMockPrefs(JSON.stringify(stored));
				for (const key of MINI_MOCK_OPTION_KEYS) {
					expect(MINI_MOCK_OPTIONS[key] as readonly string[]).toContain(prefs[key]);
				}
				expect(typeof prefs.enabled).toBe('boolean');
			})
		);
	});
});
