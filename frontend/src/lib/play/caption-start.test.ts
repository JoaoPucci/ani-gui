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

describe('startingTrack — region, then script, then language', () => {
	it('takes the exact region over a track that only shares its script', () => {
		expect(startingTrack([t('zh-Hant'), t('zh-HK')], '', 'zh-HK')).toBe(1);
		expect(startingTrack([t('pt-Latn-PT'), t('pt-BR')], 'pt-BR')).toBe(1);
	});

	it("reads a track's script from its region when the tag names none", () => {
		expect(startingTrack([t('zh-CN'), t('zh-TW')], '', 'zh-Hant')).toBe(1);
		expect(startingTrack([t('zh-CN'), t('zh-HK')], '', 'zh-TW')).toBe(1);
	});
});

describe('startingTrack — languages written in more than one script', () => {
	it('reads Serbian as Cyrillic but where its region writes it in Latin', () => {
		const tracks = [t('sr-Cyrl'), t('sr-Latn')];
		expect(startingTrack(tracks, 'sr-ME')).toBe(1);
		expect(startingTrack(tracks, 'sr')).toBe(0);
	});

	it('reads a bare Chinese tag as Simplified, by the same table the backend uses', () => {
		expect(startingTrack([t('zh-TW'), t('zh')], '', 'zh-Hans')).toBe(1);
		expect(startingTrack([t('zh-Hant'), t('zh-Hans')], '', 'zh')).toBe(1);
		expect(startingTrack([t('zh-Hans'), t('zh-Hant')], 'zh-US')).toBe(0);
	});
});

describe('startingTrack — what is not a tag', () => {
	it('reads a malformed track tag or locale as no language', () => {
		expect(startingTrack([t('not a tag!'), t('en', true)], 'pt_BR')).toBe(1);
		expect(startingTrack([t('pt-BR'), t('en', true)], 'Portuguese (Brazil)')).toBe(1);
		expect(startingTrack([t(''), t('und')], 'x')).toBeNull();
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
	'pt-Latn-PT',
	'pt',
	'es-419',
	'es-ES',
	'ja',
	'und',
	'zh-Hans',
	'zh-Hant',
	'zh-CN',
	'zh-TW',
	'zh-HK'
];
const LOCALES = ['', 'en', 'pt-BR', 'es-419', 'ru', 'ja', 'zh-TW'];

const lang = (tag: string) => tag.split('-')[0].toLowerCase();
const regionOf = (tag: string) =>
	tag
		.split('-')
		.slice(1)
		.find((s) => /^[A-Z]{2}$/.test(s) || /^\d{3}$/.test(s));
/** The script a tag is written in, spelled out here for the tags the
 *  property draws: every language but Chinese in the one script it
 *  has, named or not; Chinese by its script or its region. */
const scriptOf = (tag: string) => {
	if (lang(tag) !== 'zh') return `the ${lang(tag)} script`;
	const named = tag
		.split('-')
		.slice(1)
		.find((s) => s.length === 4);
	if (named) return named;
	const region = regionOf(tag);
	if (region === 'TW' || region === 'HK') return 'Hant';
	if (region === 'CN') return 'Hans';
	return undefined;
};

describe('startingTrack — properties', () => {
	it('remembered beats locale beats default; within each the exact region, then the script, then the language', () => {
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
						const region = regionOf(want);
						const exactAt = region
							? tracks.findIndex((x) => lang(x.lang) === lang(want) && regionOf(x.lang) === region)
							: -1;
						if (exactAt !== -1) return got === exactAt;
						const script = scriptOf(want);
						const scriptAt =
							script === undefined
								? -1
								: tracks.findIndex(
										(x) => lang(x.lang) === lang(want) && scriptOf(x.lang) === script
									);
						return got === (scriptAt === -1 ? same : scriptAt);
					}
					const def = tracks.findIndex((x) => x.default);
					return got === (def === -1 ? null : def);
				}
			)
		);
	});
});
