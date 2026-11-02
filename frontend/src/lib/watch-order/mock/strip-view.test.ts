import { describe, expect, it } from 'vitest';
import { MOCK_RUNS } from './fixtures';
import { placement } from './order';
import {
	emptyColumn,
	franchiseSummary,
	similarHidden,
	stripRunFor,
	stripView,
	watchingFor
} from './strip-view';
import type { MockEntry, MockRun } from './types';

const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;
const ids = (list: readonly MockEntry[]) => list.map((e) => e.id);

describe('stripRunFor', () => {
	it("finds a member's run, and a side story's through its parents", () => {
		expect(stripRunFor('46598')?.key).toBe('jojo');
		expect(stripRunFor('6928')?.key).toBe('fate');
		expect(stripRunFor('11759')?.key).toBe('gintama');
		expect(stripRunFor('12064')?.key).toBe('jojo');
	});

	it('finds nothing for an entry with no strip', () => {
		// A Bleach film: around the run, but no parent story recorded.
		expect(stripRunFor('1515')).toBeNull();
		expect(stripRunFor('999999')).toBeNull();
	});
});

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
});

describe('watchingFor', () => {
	const watching = (key: string, id: string, order: 'linked' | 'released' = 'linked') => {
		const r = run(key);
		const w = watchingFor(r, id, stripView(r, id, order), order);
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
