import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { KitsuAnimeRef } from '$lib/api';
import { entryNames, namesTheEntry } from './same-name';

interface Vector {
	case: string;
	title: string;
	names: string[];
	names_entry: boolean;
}

const vectors: Vector[] = JSON.parse(
	readFileSync(
		path.resolve(
			path.dirname(fileURLToPath(import.meta.url)),
			'../../../../tests/fixtures/same-name/vectors.json'
		),
		'utf8'
	)
);

describe('namesTheEntry shared vectors', () => {
	it.each(vectors.map((v) => [v.case, v] as const))('%s', (_name, v) => {
		expect(namesTheEntry(v.title, v.names)).toBe(v.names_entry);
	});

	it('runs every shared vector', () => {
		expect(vectors.length).toBeGreaterThanOrEqual(29);
	});
});

describe('entryNames', () => {
	it('skips a canonical title a stale cached shape lacks', () => {
		const ref = {
			canonical_title: null,
			titles: { en: 'Gintama Season 2' },
			abbreviated_titles: undefined
		} as unknown as KitsuAnimeRef;
		expect(entryNames(ref)).toEqual(['Gintama Season 2']);
		expect(namesTheEntry('Gintama Season 2', entryNames(ref))).toBe(true);
	});
});

describe('namesTheEntry', () => {
	it('names an entry that carries the title', () => {
		// Gintama°: Kitsu's en_us title is the provider's card title.
		expect(
			namesTheEntry('Gintama Season 4', ['Gintama°', 'Gintama Season 4', "Gintama' (2015)"])
		).toBe(true);
	});

	it('folds case and punctuation', () => {
		expect(
			namesTheEntry('Re:ZERO -Starting Life in Another World- Season 2', [
				're zero starting life in another world season 2'
			])
		).toBe(true);
	});

	it('reads the cour suffix whichever way it is spelled', () => {
		for (const kitsu of [
			'Show Season 2',
			'Show 2nd Season',
			'Show Second Season',
			'Show Part 2',
			'Show Cour 2'
		]) {
			expect(namesTheEntry('Show 2nd Season', [kitsu]), kitsu).toBe(true);
		}
	});

	it('does not name an entry of another cour number', () => {
		expect(namesTheEntry('Show Season 3', ['Show Season 2'])).toBe(false);
	});

	it('does not name an entry that shares only the number', () => {
		expect(
			namesTheEntry('Attack on Titan Season 3 Part 2', [
				'Attack on Titan Season 2',
				'Shingeki no Kyojin Season 2'
			])
		).toBe(false);
		expect(
			namesTheEntry("JoJo's Bizarre Adventure: Stone Ocean Part 3", [
				"JoJo's Bizarre Adventure Part 3"
			])
		).toBe(false);
	});

	it('names nothing with a title of no words, or a suffix with no name', () => {
		expect(namesTheEntry('', [''])).toBe(false);
		expect(namesTheEntry('!!', ['??'])).toBe(false);
		expect(namesTheEntry('Season 2', ['2nd Season'])).toBe(false);
	});
});

describe('namesTheEntry properties', () => {
	const KEYWORDS = ['Season', 'Part', 'Cour'];
	const WORDS = [
		'First',
		'Second',
		'Third',
		'Fourth',
		'Fifth',
		'Sixth',
		'Seventh',
		'Eighth',
		'Ninth',
		'Tenth'
	];
	const suffix = (n: number): string => {
		if (n % 100 >= 11 && n % 100 <= 13) return 'th';
		return ({ 1: 'st', 2: 'nd', 3: 'rd' } as Record<number, string>)[n % 10] ?? 'th';
	};
	const spellings = (name: string, n: number, kw: string): string[] => {
		const out = [`${name} ${kw} ${n}`, `${name} ${n}${suffix(n)} ${kw}`];
		if (n >= 1 && n <= WORDS.length) out.push(`${name} ${WORDS[n - 1]} ${kw}`);
		return out;
	};
	const name = fc
		.array(fc.stringMatching(/^[a-z]{2,6}$/), { minLength: 1, maxLength: 3 })
		.map((ws) => ws.join(' '));
	const cour = fc.integer({ min: 1, max: 39 });
	const kw = fc.constantFrom(...KEYWORDS);

	it('names an entry that carries the title, whatever the case and punctuation', () => {
		fc.assert(
			fc.property(
				fc.array(fc.stringMatching(/^[A-Za-z0-9]{1,6}$/), { minLength: 1, maxLength: 4 }),
				fc.array(fc.stringMatching(/^[ :!?.'-]{1,2}$/), { minLength: 4, maxLength: 4 }),
				(words, seps) => {
					const plain = words.join(' ');
					const dressed = words.map((w, i) => `${w.toUpperCase()}${seps[i]}`).join('');
					expect(namesTheEntry(plain, [plain])).toBe(true);
					expect(namesTheEntry(plain, [dressed])).toBe(true);
				}
			)
		);
	});

	it('the cour number decides, and its spelling does not', () => {
		fc.assert(
			fc.property(name, cour, cour, kw, kw, (nm, n, m, a, b) => {
				for (const provider of spellings(nm, n, a)) {
					for (const kitsu of spellings(nm, m, b)) {
						expect(namesTheEntry(provider, [kitsu]), `${provider} / ${kitsu}`).toBe(n === m);
					}
				}
			})
		);
	});

	it('a name and its extension are not one entry', () => {
		// Independent draws almost never reach this case ("attack on titan"
		// against "attack on titan final season").
		fc.assert(
			fc.property(name, name, cour, kw, (a, more, n, k) => {
				const b = `${a} ${more}`;
				for (const provider of spellings(a, n, k)) {
					for (const kitsu of spellings(b, n, k)) {
						expect(namesTheEntry(provider, [kitsu]), `${provider} / ${kitsu}`).toBe(false);
						expect(namesTheEntry(kitsu, [provider]), `${kitsu} / ${provider}`).toBe(false);
					}
				}
			})
		);
	});

	it('another name never names the entry', () => {
		fc.assert(
			fc.property(name, name, cour, kw, (a, b, n, k) => {
				fc.pre(a !== b);
				for (const provider of spellings(a, n, k)) {
					for (const kitsu of spellings(b, n, k)) {
						expect(namesTheEntry(provider, [kitsu]), `${provider} / ${kitsu}`).toBe(false);
					}
				}
			})
		);
	});
});
