import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MOCK_RUNS } from './fixtures';
import { franchiseName, withoutFormatSuffix } from './franchise-name';
import type { MockRun } from './types';

const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;
const titlesBut = (r: MockRun, id: string) =>
	[...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps]
		.filter((e) => e.id !== id)
		.map((e) => e.title);

describe('franchiseName', () => {
	it('drops the format suffix when nothing else in the franchise would read the same', () => {
		expect(
			franchiseName('Kimetsu no Yaiba (TV)', ['Kimetsu no Yaiba Movie: Mugen Ressha-hen'])
		).toBe('Kimetsu no Yaiba');
		expect(franchiseName('Fate/Zero', titlesBut(run('fate'), '6028'))).toBe('Fate/Zero');
	});

	it("keeps JoJo's (TV): the franchise's 1993 OVA is titled JoJo no Kimyou na Bouken", () => {
		const jojo = run('jojo');
		const first = jojo.entries.find((e) => e.id === jojo.nameId)!;
		expect(first.title).toBe('JoJo no Kimyou na Bouken (TV)');
		expect(franchiseName(first.title, titlesBut(jojo, first.id))).toBe(
			'JoJo no Kimyou na Bouken (TV)'
		);
	});

	it("keeps Zoku Owarimonogatari (TV): the franchise's film has the bare title", () => {
		const mono = run('monogatari');
		expect(franchiseName('Zoku Owarimonogatari (TV)', titlesBut(mono, '42279'))).toBe(
			'Zoku Owarimonogatari (TV)'
		);
	});

	it('counts a clash with another suffixed title, and ignores case', () => {
		expect(franchiseName('Bleach (TV)', ['Bleach (Movie)'])).toBe('Bleach (TV)');
		expect(franchiseName('Bleach (TV)', ['BLEACH'])).toBe('Bleach (TV)');
	});

	const suffix = fc.constantFrom('', ' (TV)', ' (Movie)', ' (OVA)', ' (ONA)', ' (Special)');
	const name = fc.constantFrom('Bleach', 'BLEACH', 'Gintama', 'Fate/Zero', 'JoJo');
	const title = fc.tuple(name, suffix).map(([n, s]) => n + s);

	it('returns the title, or the title without its suffix', () => {
		fc.assert(
			fc.property(title, fc.array(title, { maxLength: 4 }), (t, others) => {
				expect([t, withoutFormatSuffix(t)]).toContain(franchiseName(t, others));
			})
		);
	});

	it('never drops a suffix into a name another entry reads as, whatever its case or suffix', () => {
		fc.assert(
			fc.property(title, fc.array(title, { maxLength: 4 }), (t, others) => {
				const out = franchiseName(t, others);
				if (out === t) return;
				for (const o of others) {
					expect(withoutFormatSuffix(o).toLowerCase()).not.toBe(out.toLowerCase());
				}
			})
		);
	});

	it('drops the suffix when the franchise has no other title, and keeps it beside its bare name', () => {
		fc.assert(
			fc.property(
				name,
				suffix.filter((x) => x !== ''),
				(n, sfx) => {
					expect(franchiseName(n + sfx, [])).toBe(n);
					expect(franchiseName(n + sfx, [n.toUpperCase()])).toBe(n + sfx);
				}
			)
		);
	});
});
