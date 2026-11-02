import { describe, expect, it } from 'vitest';
import {
	loadMockPrefs,
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
		const p = parseMockPrefs(JSON.stringify({ enabled: true, strip: 'trio', order: 'sideways' }));
		expect(p).toMatchObject({ enabled: true, switcher: false, strip: 'trio', order: 'linked' });
	});

	it("starts the strip in the maintainer's chosen arrangement, with the new options' first choices", () => {
		expect(parseMockPrefs(null)).toMatchObject({
			strip: 'trio',
			wording: 'role',
			cta: 'button',
			resume: 'chip'
		});
	});

	it("keeps the second round's choices", () => {
		const p = parseMockPrefs(
			JSON.stringify({ wording: 'watch', cta: 'stack', resume: 'sentence', strip: 'rail' })
		);
		expect(p).toMatchObject({ wording: 'watch', cta: 'stack', resume: 'sentence', strip: 'rail' });
	});

	it('round-trips through storage', () => {
		const s = memory();
		const next = { ...MOCK_PREFS_DEFAULT, enabled: true, upNext: 'still' as const };
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
