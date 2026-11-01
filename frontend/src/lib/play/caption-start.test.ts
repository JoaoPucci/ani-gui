import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { startingTrack, type StartableTrack } from './caption-start';

const t = (lang: string, isDefault = false): StartableTrack => ({ lang, default: isDefault });

/** Steel Ball Run's first episode, in part: English is the default. */
const listing = [
	t('ar'),
	t('zh-Hans'),
	t('zh-Hant'),
	t('en', true),
	t('pt-BR'),
	t('pt-PT'),
	t('es-419'),
	t('es-ES'),
	t('und')
];

describe('startingTrack — the locale', () => {
	it("starts on the track in the user's language and region", () => {
		expect(startingTrack(listing, 'pt-BR')).toBe(4);
		expect(startingTrack(listing, 'es-419')).toBe(6);
		expect(startingTrack(listing, 'es-ES')).toBe(7);
	});

	it("takes another region of the user's language over the provider's default", () => {
		expect(startingTrack([t('en', true), t('pt-PT')], 'pt-BR')).toBe(1);
		expect(startingTrack([t('en', true), t('es-ES'), t('es-419')], 'es')).toBe(1);
	});

	it("falls back to the provider's default when nothing is in the user's language", () => {
		expect(startingTrack(listing, 'ru')).toBe(3);
		expect(startingTrack(listing, '')).toBe(3);
	});

	it('starts on nothing when nothing is in the language and the provider flags no default', () => {
		expect(startingTrack([t('ar'), t('ja')], 'ru')).toBeNull();
		expect(startingTrack([], 'en')).toBeNull();
	});

	it('never starts on a track of unknown language by the locale', () => {
		expect(startingTrack([t('und'), t('en', true)], 'und')).toBe(1);
		expect(startingTrack([t('und')], 'pt-BR')).toBeNull();
	});

	it('takes the script a region writes Chinese in', () => {
		expect(startingTrack(listing, 'zh-TW')).toBe(2);
		expect(startingTrack(listing, 'zh-CN')).toBe(1);
	});

	it('reads a three-letter code as its language', () => {
		expect(startingTrack([t('eng', true), t('por')], 'pt-BR')).toBe(1);
	});
});

describe("startingTrack — the show's remembered choice", () => {
	it('starts on the remembered language over the locale and the default', () => {
		expect(startingTrack(listing, 'pt-BR', 'es-419')).toBe(6);
		expect(startingTrack(listing, 'pt-BR', 'ja')).toBe(4);
	});

	it('finds another region of the remembered language before the locale', () => {
		expect(startingTrack([t('en', true), t('pt-BR'), t('es-ES')], 'pt-BR', 'es-419')).toBe(2);
	});

	it('falls back to the locale, then the default, when the remembered language is missing', () => {
		expect(startingTrack([t('en', true), t('pt-BR')], 'pt-BR', 'ja')).toBe(1);
		expect(startingTrack([t('en', true), t('ar')], 'pt-BR', 'ja')).toBe(0);
	});

	it('stays off when off is remembered', () => {
		expect(startingTrack(listing, 'pt-BR', 'off')).toBeNull();
	});
});

const TAGS = [
	'en',
	'en-US',
	'pt-BR',
	'pt-PT',
	'pt',
	'es-419',
	'es-ES',
	'ja',
	'und',
	'zh-Hans',
	'zh-Hant'
];
const LOCALES = ['', 'en', 'pt-BR', 'es-419', 'ru', 'ja', 'zh-TW'];

const lang = (tag: string) => tag.split('-')[0].toLowerCase();
const exact = (track: string, want: string) => {
	const [l, ...rest] = want.split('-');
	const [tl, ...trest] = track.split('-');
	if (l.toLowerCase() !== tl.toLowerCase()) return false;
	const region = rest.find((s) => s.length === 2 || /^\d{3}$/.test(s));
	const script =
		region === 'TW' ? 'Hant' : region === 'CN' ? 'Hans' : rest.find((s) => s.length === 4);
	return trest.some((s) => s === region || s === script);
};

describe('startingTrack — properties', () => {
	it('remembered beats locale beats default, the exact region first within each', () => {
		fc.assert(
			fc.property(
				fc.array(fc.record({ lang: fc.constantFrom(...TAGS), default: fc.boolean() }), {
					maxLength: 15
				}),
				fc.constantFrom(...LOCALES),
				fc.option(fc.constantFrom('off', ...TAGS.filter((x) => x !== 'und')), { nil: undefined }),
				(tracks, locale, remembered) => {
					const got = startingTrack(tracks, locale, remembered);
					if (remembered === 'off') return got === null;
					const tiers = [remembered, locale].filter((w): w is string => !!w && lang(w) !== 'und');
					for (const want of tiers) {
						const same = tracks.findIndex((x) => lang(x.lang) === lang(want));
						if (same === -1) continue;
						const precise = tracks.findIndex((x) => exact(x.lang, want));
						return got === (precise === -1 ? same : precise);
					}
					const def = tracks.findIndex((x) => x.default);
					return got === (def === -1 ? null : def);
				}
			)
		);
	});
});
