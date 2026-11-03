import { describe, expect, it } from 'vitest';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import { placement } from './order';
import {
	currentIn,
	emptyColumn,
	franchiseSummary,
	similarHidden,
	stripView,
	watchingFor
} from './strip-view';
import type { WatchEntry, WatchRun } from './types';

const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
const ids = (list: readonly WatchEntry[]) => list.map((e) => e.id);

describe('stripView', () => {
	it("is a member's placement in its run", () => {
		for (const order of ['linked', 'released'] as const) {
			const p = placement(run('fate'), '7658', order);
			expect(stripView(run('fate'), '7658', order)).toEqual({ kind: 'run', ...p });
		}
	});

	it("shows a side story's parent stories before it, nothing after, and no number", () => {
		for (const order of ['linked', 'released'] as const) {
			const v = stripView(run('fate'), '6928', order);
			expect(v.kind).toBe('side');
			expect(ids(v.before)).toEqual(['6028', '7658']);
			expect(v.after).toEqual([]);
			expect(v.position).toBeNull();
			expect(v.total).toBeNull();
			expect(v.inCycle).toBe(false);
		}
		expect(ids(stripView(run('gintama'), '11759', 'linked').before)).toEqual(['10083']);
	});
});

describe('emptyColumn', () => {
	it('says the run starts or ends, or that nothing follows a side story', () => {
		expect(emptyColumn(stripView(run('bleach'), '244', 'linked'), 'before')).toBe('first');
		expect(emptyColumn(stripView(run('bleach'), '49444', 'linked'), 'after')).toBe('last');
		expect(emptyColumn(stripView(run('jojo'), '12064', 'linked'), 'after')).toBe('side_end');
	});
});

describe('franchiseSummary', () => {
	it('counts the main story, films with other versions, and side stories', () => {
		expect(franchiseSummary(run('fate'))).toMatchObject({ main: 6, films: 3, sides: 1 });
		expect(franchiseSummary(run('bleach'))).toMatchObject({ main: 5, films: 4, sides: 3 });
	});

	it('totals every entry the franchise page lists, recaps included', () => {
		// JoJo: 9 in the run, 4 other versions, 1 side story, 1 recap.
		expect(franchiseSummary(run('jojo')).total).toBe(15);
		expect(franchiseSummary(run('fate')).total).toBe(12);
	});

	it('counts the recaps, so the stack lists what the total counts', () => {
		expect(franchiseSummary(run('jojo')).recaps).toBe(1);
		expect(franchiseSummary(run('fate')).recaps).toBe(2);
	});

	it('fans the first entry, a film and a side story, filling from the main story', () => {
		expect(ids(franchiseSummary(run('fate')).posters)).toEqual(['6028', '4788', '6928']);
		expect(ids(franchiseSummary(run('monogatari')).posters)).toEqual(['3919', '49128', '6546']);
	});

	it('fans only entries that have a poster, and none when no entry has one', () => {
		const fate = run('fate');
		const blank = (e: WatchEntry): WatchEntry => ({ ...e, poster: null });
		const noFirst: WatchRun = {
			...fate,
			entries: fate.entries.map((e) => (e.id === fate.nameId ? blank(e) : e))
		};
		expect(ids(franchiseSummary(noFirst).posters)).toEqual(['4788', '6928', '7658']);
		const none: WatchRun = {
			...fate,
			entries: fate.entries.map(blank),
			versions: fate.versions.map(blank),
			films: fate.films.map(blank),
			sides: fate.sides.map(blank),
			recaps: fate.recaps.map(blank)
		};
		expect(franchiseSummary(none).posters).toEqual([]);
	});
});

describe('stripView in release order', () => {
	it("puts every Monogatari entry on one numbered line, 'N of 16'", () => {
		const mono = run('monogatari');
		const positions = mono.entries.map((e) => stripView(mono, e.id, 'released'));
		for (const v of positions) {
			expect(v.total).toBe(16);
			expect(v.position).not.toBeNull();
			expect(v.inCycle).toBe(false);
			expect(v.before.length).toBeLessThanOrEqual(1);
			expect(v.after.length).toBeLessThanOrEqual(1);
		}
		expect(positions.map((v) => v.position).sort((a, b) => a! - b!)).toEqual(
			Array.from({ length: 16 }, (_, i) => i + 1)
		);
		// Nisemonogatari is second by date.
		expect(stripView(mono, '6546', 'released')).toMatchObject({ position: 2, total: 16 });
	});

	it('gives no total in link order, where the run is a loop', () => {
		const mono = run('monogatari');
		for (const e of mono.entries) expect(stripView(mono, e.id, 'linked').total).toBeNull();
	});
});

describe('currentIn', () => {
	it('names the entry the viewer is on', () => {
		const jojo = run('jojo');
		expect(currentIn(jojo, jojo.progress, 'linked')).toMatchObject({
			entry: { id: '49847' },
			episode: 1
		});
		expect(currentIn(run('bleach'), run('bleach').progress, 'linked')).toBeNull();
	});
});

describe('watchingFor', () => {
	const watching = (key: string, id: string, order: 'linked' | 'released' = 'linked') => {
		const r = run(key);
		const w = watchingFor(r, r.progress, id, stripView(r, id, order), order);
		return w && [w.entry.id, w.episode];
	};

	it('names the entry the viewer is part-way through, elsewhere in the run', () => {
		// On Golden Wind, the viewer is on Steel Ball Run, episode 1.
		expect(watching('jojo', '41410')).toEqual(['49847', 1]);
		expect(watching('gintama', '43683')).toEqual(['818', 6]);
	});

	it('says nothing on that entry itself, or where a tile beside it already shows it', () => {
		expect(watching('jojo', '49847')).toBeNull();
		// Steel Ball Run is Stone Ocean Part 3's sequel tile.
		expect(watching('jojo', '46598')).toBeNull();
		// Fate/Zero 2nd Season is one of the side story's parent tiles.
		expect(watching('fate', '6928')).toBeNull();
	});

	it('says nothing where nothing is part-way', () => {
		expect(watching('bleach', '244')).toBeNull();
	});
});

describe('similarHidden', () => {
	it("leaves out the run's entries and the strip's tiles, and nothing else", () => {
		const jojo = run('jojo');
		const hidden = similarHidden(jojo, stripView(jojo, '12064', 'linked'));
		for (const e of jojo.entries) expect(hidden.has(e.id)).toBe(true);
		expect(hidden.has('11459')).toBe(true);
		// The franchise's other versions are not the run: they may still appear.
		expect(hidden.has('610')).toBe(false);
		expect(hidden.size).toBe(jojo.entries.length);
	});
});
