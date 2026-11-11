import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import { franchiseName, nameEntry, withoutFormatSuffix } from './franchise-name';
import { sequence } from './order';
import { runOf } from './strip-model';
import { entriesArb, linksArb } from './testing/arbitraries';
import type { RunLink, WatchEntry, WatchRun } from './types';

const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
const titlesBut = (r: FixtureRun, id: string) =>
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

/** An entry with only what the name reads. */
const e = (
	id: string,
	subtype: string | null,
	start: string | null,
	title = `T${id}`
): WatchEntry => ({
	id,
	title,
	subtype,
	start,
	episodes: null,
	status: null,
	poster: null,
	cover: null
});

/** The run `entries` and `links` make. */
const runFrom = (entries: WatchEntry[], links: RunLink[] = []): WatchRun =>
	runOf({
		anchor: entries[0].id,
		entries,
		links,
		versions: [],
		films: [],
		sides: [],
		recaps: [],
		parents: {},
		complete: true
	})!;

describe('nameEntry', () => {
	it('skips a pilot film before the first TV series: Lupin III, not its 1969 pilot', () => {
		const r = runFrom(
			[
				e('3997', 'movie', '1969-01-01', 'Lupin III: Pilot Film'),
				e('1265', 'TV', '1971-10-24', 'Lupin III'),
				e('1278', 'TV', '1977-10-03', 'Lupin III: Part II'),
				e('46654', 'ONA', '2022-12-16', 'LUPIN ZERO')
			],
			[
				['3997', '1265'],
				['46654', '1265'],
				['1265', '1278']
			]
		);
		expect(nameEntry(r).id).toBe('1265');
	});

	it('skips OVAs, specials and ONAs as well as films', () => {
		for (const sub of ['OVA', 'special', 'ONA', 'movie', 'music', null]) {
			const r = runFrom([e('1', sub, '2000-01-01'), e('2', 'TV', '2003-01-01')], [['1', '2']]);
			expect(nameEntry(r).id).toBe('2');
		}
	});

	it('takes the first TV series in link order, not the earliest released', () => {
		// FLCL: the OVA, then Progressive, then Alternative by the links,
		// though Alternative aired first.
		const r = runFrom(
			[
				e('202', 'OVA', '2000-04-26'),
				e('11962', 'TV', '2018-03-31'),
				e('11961', 'TV', '2018-06-02')
			],
			[
				['202', '11961'],
				['11961', '11962']
			]
		);
		expect(nameEntry(r).id).toBe('11961');
	});

	it('keeps the first title by its links where the run holds no TV series', () => {
		const r = runFrom(
			[
				e('2357', 'movie', '2007-12-01'),
				e('3248', 'movie', '2007-12-29'),
				e('4804', 'special', '2011-02-02')
			],
			[
				['2357', '3248'],
				['3248', '4804']
			]
		);
		expect(nameEntry(r).id).toBe(r.nameId);
		expect(nameEntry(r).id).toBe('2357');
	});

	it('reads only the main story: a TV series among the versions does not name it', () => {
		const r: WatchRun = {
			...runFrom(
				[e('1162', 'OVA', '1988-04-25'), e('980', 'movie', '1989-07-15')],
				[['1162', '980']]
			),
			versions: [e('291', 'TV', '1989-10-11')]
		};
		expect(nameEntry(r).id).toBe('1162');
	});

	const anyRun = entriesArb(1, 7).chain((entries) =>
		linksArb(entries.map((x) => x.id)).map((links) => runFrom(entries, links))
	);

	it('names a run after its first TV series in link order, else after its first title', () => {
		fc.assert(
			fc.property(anyRun, (r) => {
				const got = nameEntry(r);
				expect(r.entries).toContain(got);
				const line = sequence(r, 'linked');
				const tv = line.find((x) => x.subtype === 'TV');
				expect(got).toBe(tv ?? r.entries.find((x) => x.id === r.nameId));
			})
		);
	});

	it('does not depend on the order setting or on the order entries are listed in', () => {
		fc.assert(
			fc.property(anyRun, fc.boolean(), (r, flip) => {
				const shuffled = runFrom(flip ? [...r.entries].reverse() : [...r.entries], [...r.links]);
				expect(nameEntry(shuffled).id).toBe(nameEntry(r).id);
			})
		);
	});
});
