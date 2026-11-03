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
		const p = parseMockPrefs(JSON.stringify({ enabled: true, strip: 'trio', order: 'sideways' }));
		expect(p).toMatchObject({ enabled: true, switcher: false, strip: 'trio', order: 'linked' });
	});

	it("starts the strip in the maintainer's chosen arrangement", () => {
		expect(parseMockPrefs(null)).toMatchObject({ strip: 'trio', cta: 'stack' });
	});

	it('keeps the stack and rail choices', () => {
		const p = parseMockPrefs(JSON.stringify({ cta: 'button', strip: 'rail' }));
		expect(p).toMatchObject({ cta: 'button', strip: 'rail' });
	});

	it('offers no switch for a choice the maintainer has made', () => {
		// Relation wording, the watching card, the strip under the
		// masthead and short titles were decided in the first two
		// review rounds; the real strip has no other option to show.
		for (const decided of ['wording', 'resume', 'placement', 'titles']) {
			expect(Object.keys(MOCK_OPTIONS)).not.toContain(decided);
			expect(Object.keys(parseMockPrefs(null))).not.toContain(decided);
		}
	});

	it("starts the third round on the maintainer's poster stack, centred in the footer, with every poster", () => {
		expect(parseMockPrefs(null)).toMatchObject({
			cta: 'stack',
			stackHover: 'surface',
			footer: 'center',
			stackPosters: '3'
		});
	});

	it("keeps the third round's choices", () => {
		for (const [stackHover, footer, stackPosters] of [
			['underline', 'axis', '2'],
			['border', 'ends', '1'],
			['surface', 'center', '0']
		]) {
			expect(parseMockPrefs(JSON.stringify({ stackHover, footer, stackPosters }))).toMatchObject({
				stackHover,
				footer,
				stackPosters
			});
		}
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
