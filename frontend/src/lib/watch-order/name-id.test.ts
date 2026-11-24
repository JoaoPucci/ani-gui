import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { byStart } from './by-start';
import { FIXTURE_RUNS } from './fixtures/runs';
import { nameId } from './name-id';
import { entriesArb, linksArb } from './testing/arbitraries';

const scenario = entriesArb().chain((entries) =>
	fc.record({
		entries: fc.constant(entries),
		links: linksArb(entries.map((e) => e.id))
	})
);

describe('nameId', () => {
	it("finds each fixture franchise's first entry", () => {
		// The 2004 Bleach series, the 2012 JoJo series, the 2006
		// Gintama (not The Semi-Final, which no prequel reaches but
		// which started later), Fate/Zero, Bakemonogatari.
		for (const run of FIXTURE_RUNS) expect(nameId(run.entries, run.links)).toBe(run.nameId);
	});

	it('takes the earliest member of a run that is one loop', () => {
		const [a, b] = FIXTURE_RUNS.find((r) => r.key === 'bleach')!.entries;
		expect(
			nameId(
				[b, a],
				[
					[a.id, b.id],
					[b.id, a.id]
				]
			)
		).toBe(a.id);
	});

	it('names nothing in an empty run', () => {
		expect(nameId([], [])).toBeNull();
	});

	it('is a member no prequel reaches, the earliest of them, whenever there is one', () => {
		fc.assert(
			fc.property(scenario, ({ entries, links }) => {
				const got = nameId(entries, links)!;
				const reached = new Set(links.filter(([a, b]) => a !== b).map(([, b]) => b));
				const roots = entries.filter((e) => !reached.has(e.id));
				const pool = roots.length > 0 ? roots : entries;
				const first = [...pool].sort(byStart)[0];
				expect(got).toBe(first.id);
			})
		);
	});

	it('does not depend on the order entries and links are listed in', () => {
		fc.assert(
			fc.property(
				scenario.chain((s) =>
					fc.record({
						s: fc.constant(s),
						entries: fc.shuffledSubarray(s.entries, {
							minLength: s.entries.length,
							maxLength: s.entries.length
						}),
						links: fc.shuffledSubarray(s.links, {
							minLength: s.links.length,
							maxLength: s.links.length
						})
					})
				),
				({ s, entries, links }) => {
					expect(nameId(entries, links)).toBe(nameId(s.entries, s.links));
				}
			)
		);
	});
});
