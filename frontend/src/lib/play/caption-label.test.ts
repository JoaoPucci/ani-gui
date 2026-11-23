import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { captionEntries, cleanLabel, isSdh, languageName } from './caption-label';

const format = {
	numbered: (name: string, n: number) => `${name} ${n}`,
	untitled: (position: number) => `Track ${position}`,
	qualified: (name: string, detail: string) => `${name} · ${detail}`
};

/** Whether every bracket in `text` closes, in order and in kind. */
function balanced(text: string): boolean {
	const open: string[] = [];
	for (const ch of text) {
		if (ch === '(' || ch === '[') open.push(ch);
		else if (ch === ')' && open.pop() !== '(') return false;
		else if (ch === ']' && open.pop() !== '[') return false;
	}
	return open.length === 0;
}

describe('isSdh — a track for the deaf and hard of hearing', () => {
	it('reads the SDH mark the provider puts in a label', () => {
		expect(isSdh('Spanish (- Latin America | SDH)')).toBe(true);
		expect(isSdh('Thai (- SDH)')).toBe(true);
		expect(isSdh('English [sdh]')).toBe(true);
		expect(isSdh('English (Hearing Impaired)')).toBe(true);
		expect(isSdh('English hearing-impaired')).toBe(true);
	});

	it('leaves every other label alone, a word merely containing the letters too', () => {
		expect(isSdh('Spanish (- Latin America)')).toBe(false);
		expect(isSdh('Thai')).toBe(false);
		expect(isSdh('SDHX')).toBe(false);
		expect(isSdh('')).toBe(false);
	});
});

describe("cleanLabel — the provider's label without its punctuation", () => {
	it('drops the dash the provider opens a qualifier with', () => {
		expect(cleanLabel('Portuguese (- Brazil)')).toBe('Portuguese (Brazil)');
		expect(cleanLabel('Chinese (- Traditional)')).toBe('Chinese (Traditional)');
	});

	it('drops the SDH mark, and a qualifier it leaves empty', () => {
		expect(cleanLabel('Spanish (- Latin America | SDH)')).toBe('Spanish (Latin America)');
		expect(cleanLabel('Thai (- SDH)')).toBe('Thai');
		expect(cleanLabel('English [SDH]')).toBe('English');
		expect(cleanLabel('SDH')).toBe('');
	});

	it('keeps a label with nothing to clean as it is', () => {
		expect(cleanLabel('Norwegian (Bokmål)')).toBe('Norwegian (Bokmål)');
		expect(cleanLabel('English')).toBe('English');
	});

	it('drops a bracket the label never closes or never opened', () => {
		expect(cleanLabel('Thai (- SDH')).toBe('Thai');
		expect(cleanLabel('(SDH')).toBe('');
		expect(cleanLabel('English) (Forced')).toBe('English Forced');
		expect(cleanLabel('Signs ((Forced))')).toBe('Signs ((Forced))');
	});

	it('joins the qualifiers a label lists', () => {
		expect(cleanLabel('Spanish (- Latin America | Forced)')).toBe(
			'Spanish (Latin America, Forced)'
		);
	});

	it('is idempotent, and leaves no SDH mark, empty brackets or stray dash', () => {
		const piece = fc.constantFrom(
			'Spanish',
			'(',
			')',
			'[',
			']',
			'- ',
			' | ',
			'SDH',
			'sdh',
			'Hearing Impaired',
			'Latin America',
			' ',
			',',
			'–'
		);
		fc.assert(
			fc.property(fc.array(piece, { maxLength: 12 }), (pieces) => {
				const once = cleanLabel(pieces.join(''));
				expect(cleanLabel(once)).toBe(once);
				expect(isSdh(once)).toBe(false);
				expect(once).not.toMatch(/\(\s*\)|\[\s*\]|\(\s*[-–]|^[\s\-–|,;]|[\s\-–|,;]$/);
				expect(balanced(once)).toBe(true);
			})
		);
	});
});

describe('languageName — a tag named in the app language', () => {
	it('names the language and its region or script', () => {
		expect(languageName('pt-BR', 'en')).toBe('Portuguese (Brazil)');
		expect(languageName('es-419', 'en')).toBe('Spanish (Latin America)');
		expect(languageName('zh-Hant', 'en')).toBe('Chinese (Traditional)');
		expect(languageName('en', 'en')).toBe('English');
	});

	it("names it in the app's own language, capitalised as a menu entry", () => {
		expect(languageName('pt-BR', 'pt-BR')).toBe('Português (Brasil)');
		expect(languageName('es-419', 'es-419')).toBe('Español (Latinoamérica)');
		expect(languageName('en', 'ru')).toBe('Английский');
	});

	it('names nothing for a tag of no language or a language it cannot name', () => {
		expect(languageName('und', 'en')).toBeNull();
		expect(languageName('', 'en')).toBeNull();
		expect(languageName('not a tag', 'en')).toBeNull();
		expect(languageName('qaa', 'en')).toBeNull();
	});

	it('names a tag that carries an extension or a private-use part by its language', () => {
		expect(languageName('en-x-foo', 'en')).toBe('English');
		expect(languageName('en-u-nu-thai', 'en')).toBe('English');
		expect(languageName('pt-BR-t-ja', 'en')).toBe('Portuguese (Brazil)');
	});

	it('falls back to the default language for a locale it cannot read', () => {
		expect(languageName('fr', '')).toBe(languageName('fr', 'en'));
		expect(languageName('fr', '!!')).toBe(languageName('fr', 'en'));
	});
});

describe('captionEntries — what each track reads as in the menu', () => {
	it('names a tagged track from its tag, not its label', () => {
		const [pt] = captionEntries(
			[{ index: 3, lang: 'pt-BR', label: 'Portuguese (- Brazil)' }],
			'pt-BR',
			format
		);
		expect(pt).toMatchObject({ index: 3, name: 'Português (Brasil)', sdh: false });
	});

	it('marks an SDH track rather than spelling it out in the name', () => {
		const [es] = captionEntries(
			[{ index: 0, lang: 'es-419', label: 'Spanish (- Latin America | SDH)' }],
			'en',
			format
		);
		expect(es).toMatchObject({ name: 'Spanish (Latin America)', sdh: true });
	});

	it("falls back to the cleaned label, then the track's position", () => {
		const entries = captionEntries(
			[
				{ index: 0, lang: 'und', label: 'Signs (- Forced)' },
				{ index: 1, lang: '', label: '' },
				{ index: 4, lang: 'und', label: 'SDH' }
			],
			'en',
			format
		);
		expect(entries.map((e) => [e.name, e.sdh])).toEqual([
			['Signs (Forced)', false],
			['Track 2', false],
			['Track 5', true]
		]);
	});

	it("tells tracks of one language apart by the provider's qualifier before numbering them", () => {
		const entries = captionEntries(
			[
				{ index: 0, lang: 'en', label: 'English' },
				{ index: 1, lang: 'en', label: 'English (Signs & Songs)' },
				{ index: 2, lang: 'en', label: 'English [Forced]' },
				{ index: 3, lang: 'pt-BR', label: 'Portuguese (- Brazil)' },
				{ index: 4, lang: 'pt-BR', label: 'Portuguese (Portuguese - Brazilian)' }
			],
			'en',
			format
		);
		expect(entries.map((e) => e.name)).toEqual([
			'English',
			'English · Signs & Songs',
			'English · Forced',
			'Portuguese (Brazil)',
			'Portuguese (Brazil) · Portuguese - Brazilian'
		]);
	});

	it('qualifies by what tells the tracks apart, not by what they share', () => {
		const tracks = [
			{ index: 0, lang: 'es-419', label: 'Spanish (- Latin America)' },
			{ index: 1, lang: 'es-419', label: 'Spanish (- Latin America | Forced)' }
		];
		expect(captionEntries(tracks, 'pt-BR', format).map((e) => e.name)).toEqual([
			'Espanhol (América Latina)',
			'Espanhol (América Latina) · Forced'
		]);
		expect(captionEntries(tracks, 'en', format).map((e) => e.name)).toEqual([
			'Spanish (Latin America)',
			'Spanish (Latin America) · Forced'
		]);
	});

	it('numbers entries that would otherwise read the same', () => {
		const entries = captionEntries(
			[
				{ index: 0, lang: 'pt-BR', label: 'Portuguese (- Brazil)' },
				{ index: 1, lang: 'pt-BR', label: 'Portuguese (Brazil)' },
				{ index: 2, lang: 'th', label: 'Thai' },
				{ index: 3, lang: 'th', label: 'Thai (- SDH)' }
			],
			'en',
			format
		);
		expect(entries.map((e) => e.name)).toEqual([
			'Portuguese (Brazil) 1',
			'Portuguese (Brazil) 2',
			'Thai',
			'Thai'
		]);
	});

	it('keeps every track, in order, with a name no other entry with the same mark shares', () => {
		const lang = fc.constantFrom(
			'en',
			'pt-BR',
			'pt-PT',
			'es-419',
			'und',
			'',
			'zz-ZZ',
			'th',
			'en-x-foo',
			'en-u-nu-thai'
		);
		const label = fc.constantFrom('', 'English', 'Thai (- SDH)', 'SDH', 'X (- Y)', 'Track');
		fc.assert(
			fc.property(
				fc.array(fc.record({ lang, label }), { maxLength: 12 }),
				fc.constantFrom('en', 'pt-BR', 'ru'),
				(tracks, locale) => {
					const listed = tracks.map((t, index) => ({ ...t, index }));
					const entries = captionEntries(listed, locale, format);
					expect(entries.map((e) => e.index)).toEqual(listed.map((t) => t.index));
					const keys = entries.map((e) => `${e.name}|${e.sdh}`);
					expect(new Set(keys).size).toBe(keys.length);
					for (const e of entries) expect(e.name.trim()).not.toBe('');
				}
			)
		);
	});
});
