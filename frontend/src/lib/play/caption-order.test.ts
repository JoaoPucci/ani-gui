import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { captionOrder, type OrderableEntry } from './caption-order';

const e = (index: number, lang: string, name: string, sdh = false): OrderableEntry => ({
	index,
	lang,
	name,
	sdh
});

/** Part of Steel Ball Run's first episode, named in English. */
const listing = [
	e(0, 'ar', 'Arabic'),
	e(1, 'zh-Hans', 'Chinese (Simplified)'),
	e(2, 'en', 'English'),
	e(3, 'pt-BR', 'Portuguese (Brazil)'),
	e(4, 'pt-PT', 'Portuguese (Portugal)'),
	e(5, 'es-419', 'Spanish (Latin America)'),
	e(6, 'es-419', 'Spanish (Latin America)', true),
	e(7, 'es-ES', 'Spanish (Spain)'),
	e(8, 'th', 'Thai', true),
	e(9, 'th', 'Thai'),
	e(10, 'und', 'Commentary')
];

const names = (list: readonly OrderableEntry[]) =>
	list.map((x) => (x.sdh ? `${x.name} SDH` : x.name));

describe("captionOrder — the viewer's languages, then the rest", () => {
	it("puts the app language's tracks first, its own region leading", () => {
		const { yours, rest } = captionOrder(listing, { locale: 'pt-BR' });
		expect(names(yours)).toEqual(['Portuguese (Brazil)', 'Portuguese (Portugal)']);
		expect(names(rest)).toEqual([
			'Arabic',
			'Chinese (Simplified)',
			'Commentary',
			'English',
			'Spanish (Latin America)',
			'Spanish (Latin America) SDH',
			'Spanish (Spain)',
			'Thai',
			'Thai SDH'
		]);
	});

	it('puts the language last picked on the show ahead of the app language', () => {
		const { yours } = captionOrder(listing, { locale: 'en', remembered: 'es-419' });
		expect(names(yours)).toEqual([
			'Spanish (Latin America)',
			'Spanish (Latin America) SDH',
			'Spanish (Spain)',
			'English'
		]);
	});

	it('reads Off remembered as no language', () => {
		const { yours } = captionOrder(listing, { locale: 'en', remembered: 'off' });
		expect(names(yours)).toEqual(['English']);
	});

	it('has nothing of yours when no track is in your languages', () => {
		const { yours, rest } = captionOrder(listing, { locale: 'ru' });
		expect(yours).toEqual([]);
		expect(rest).toHaveLength(listing.length);
		expect(captionOrder(listing, { locale: '' }).yours).toEqual([]);
		expect(names(captionOrder(listing, { locale: '!!' }).rest)).toEqual(names(rest));
	});

	it('sorts the rest by name in the app language, accents and case aside', () => {
		const { rest } = captionOrder(
			[e(0, 'x', 'zulu'), e(1, 'x', 'Árabe'), e(2, 'x', 'alemão'), e(3, 'x', 'Búlgaro')],
			{ locale: 'pt-BR' }
		);
		expect(names(rest)).toEqual(['alemão', 'Árabe', 'Búlgaro', 'zulu']);
	});

	const entry = fc.record({
		index: fc.nat(40),
		lang: fc.constantFrom('en', 'en-GB', 'pt-BR', 'pt-PT', 'es-419', 'th', 'und', '', '??'),
		name: fc.constantFrom('English', 'Thai', 'Árabe', 'arabic', 'Zulu', 'Português (Brasil)'),
		sdh: fc.boolean()
	});
	const prefs = fc.record({
		locale: fc.constantFrom('en', 'pt-BR', 'es-419', 'ru', ''),
		remembered: fc.option(fc.constantFrom('en', 'pt-PT', 'th', 'off', 'und'), { nil: undefined })
	});
	const uniqueEntries = fc.uniqueArray(entry, { selector: (x) => x.index, maxLength: 15 });

	it('lists every entry exactly once', () => {
		fc.assert(
			fc.property(uniqueEntries, prefs, (entries, p) => {
				const { yours, rest } = captionOrder(entries, p);
				const out = [...yours, ...rest].map((x) => x.index).sort((a, b) => a - b);
				expect(out).toEqual(entries.map((x) => x.index).sort((a, b) => a - b));
			})
		);
	});

	it('does not depend on the order the tracks were listed in', () => {
		fc.assert(
			fc.property(
				uniqueEntries.chain((xs) =>
					fc.tuple(fc.constant(xs), fc.shuffledSubarray(xs, { minLength: xs.length }))
				),
				prefs,
				([listed, shuffled], p) => {
					expect(captionOrder(shuffled, p)).toEqual(captionOrder(listed, p));
				}
			)
		);
	});

	it('keeps the rest in name order', () => {
		fc.assert(
			fc.property(uniqueEntries, prefs, (entries, p) => {
				const { rest } = captionOrder(entries, p);
				const collator = new Intl.Collator(p.locale || 'en', { sensitivity: 'base' });
				for (let i = 1; i < rest.length; i++) {
					expect(collator.compare(rest[i - 1].name, rest[i].name)).toBeLessThanOrEqual(0);
				}
			})
		);
	});
});
