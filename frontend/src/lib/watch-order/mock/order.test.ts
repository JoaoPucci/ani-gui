import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MOCK_RUNS } from './fixtures';
import {
	byStart,
	linkGraph,
	loopSize,
	pathNote,
	placement,
	railItems,
	segments,
	sequence,
	type Segment
} from './order';
import type { MockEntry, MockRun } from './types';

const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;
const ids = (list: readonly MockEntry[]) => list.map((e) => e.id);

function entry(id: string, start: string | null): MockEntry {
	return {
		id,
		title: id,
		subtype: 'TV',
		start,
		episodes: 12,
		status: 'finished',
		poster: null,
		cover: null
	};
}

describe('byStart', () => {
	it('puts unknown dates last and breaks ties by id', () => {
		const list = [
			entry('9', null),
			entry('3', '2020-01-01'),
			entry('2', '2020-01-01'),
			entry('1', '2019-01-01'),
			entry('8', null)
		];
		expect(ids(list.sort(byStart))).toEqual(['1', '2', '3', '8', '9']);
	});
});

describe('link order', () => {
	it('a linear run numbers every entry and shows a total', () => {
		const p = placement(run('bleach'), '48015', 'linked');
		expect(p).toMatchObject({ position: 4, total: 5, inCycle: false });
		expect(ids(p.before)).toEqual(['46903']);
		expect(ids(p.after)).toEqual(['49444']);
	});

	it('ignores the shortcut from Enchousen straight to Gintama°', () => {
		const g = linkGraph(run('gintama'));
		expect(ids(g.next.get('7253')!)).toEqual(['7241']);
		expect(ids(g.prev.get('10083')!)).toEqual(['7241']);
	});

	it('The Final is a join with no total, numbered along the earliest-starting prequel', () => {
		const p = placement(run('gintama'), '43683', 'linked');
		expect(ids(p.before)).toEqual(['41079', '43829']);
		expect(p.total).toBeNull();
		expect(p.position).toBe(10);
	});

	it('Fate/Zero 2nd Season forks, and entries after the fork are unnumbered', () => {
		const fate = run('fate');
		expect(ids(placement(fate, '7658', 'linked').after)).toEqual(['323', '9718']);
		expect(placement(fate, '7658', 'linked').position).toBe(2);
		expect(placement(fate, '7882', 'linked').position).toBeNull();
	});

	it("Monogatari's loop is a cycle whose members are unnumbered", () => {
		const mono = run('monogatari');
		const g = linkGraph(mono);
		expect(g.cycle.size).toBe(10);
		expect(placement(mono, '6546', 'linked')).toMatchObject({ position: null, inCycle: true });
		expect(placement(mono, '3919', 'linked').position).toBe(1);
	});

	it('the rail puts Bakemonogatari first and the loop as one block', () => {
		const seq = ids(sequence(run('monogatari'), 'linked'));
		expect(seq[0]).toBe('3919');
		expect(seq.at(-1)).toBe('50013');
		expect(seq).toHaveLength(16);
	});
});

describe('release order', () => {
	it('is one dated line with neighbours on either side', () => {
		const p = placement(run('gintama'), '43683', 'released');
		expect(ids(p.before)).toEqual(['41079']);
		expect(ids(p.after)).toEqual(['43829']);
		expect(p).toMatchObject({ position: 10, total: 11 });
	});

	it('the first entry has nothing before it', () => {
		expect(placement(run('jojo'), '7158', 'released').before).toEqual([]);
	});
});

describe('every fixture run', () => {
	it('lists every member once in either order', () => {
		fc.assert(
			fc.property(
				fc.constantFrom(...MOCK_RUNS),
				fc.constantFrom('linked' as const, 'released' as const),
				(r, order) => {
					const seq = ids(sequence(r, order));
					expect(new Set(seq).size).toBe(r.entries.length);
					expect(seq).toHaveLength(r.entries.length);
				}
			)
		);
	});

	it('link order keeps every link of a linear run', () => {
		const aot = run('aot');
		const g = linkGraph(aot);
		const kept = [...g.next.values()].reduce((n, l) => n + l.length, 0);
		expect(kept).toBe(aot.links.length);
	});
});

describe('segments and notes', () => {
	it('lays Fate/Zero out as a line, then a fork of two branches', () => {
		const segs = segments(run('fate'), 'linked');
		expect(segs.map((s) => s.kind)).toEqual(['line', 'fork']);
		const fork = segs[1] as Extract<Segment, { kind: 'fork' }>;
		expect(fork.from.id).toBe('7658');
		expect(fork.branches.map(ids)).toEqual([['323'], ['9718', '7882', '10052']]);
	});

	it('takes Monogatari’s loop as one block, with no empty fork before it', () => {
		const kinds = segments(run('monogatari'), 'linked').map((s) => s.kind);
		expect(kinds).toEqual(['line', 'cycle', 'line']);
	});

	it('is one line in release order', () => {
		expect(segments(run('monogatari'), 'released')).toHaveLength(1);
	});

	it('does not number The Semi-Final, which leads into The Final from outside the main line', () => {
		const g = run('gintama');
		expect(placement(g, '43829', 'linked').position).toBeNull();
		expect(pathNote(g, '43829', 'linked')).toMatchObject({
			kind: 'leads-into',
			entry: { id: '43683' }
		});
		expect(pathNote(g, '41079', 'linked')).toBeNull();
		expect(pathNote(g, '43829', 'released')).toBeNull();
	});

	it('names a branch head and a loop member', () => {
		expect(pathNote(run('fate'), '9718', 'linked')).toEqual({ kind: 'branch' });
		expect(pathNote(run('fate'), '7882', 'linked')).toBeNull();
		expect(pathNote(run('monogatari'), '6546', 'linked')).toEqual({ kind: 'cycle' });
	});
});

describe('a cycle in link order', () => {
	it('follows Monogatari’s links from where the run enters the loop', () => {
		const seq = ids(sequence(run('monogatari'), 'linked'));
		const loop = seq.slice(1, 11);
		expect(loop).toEqual([
			'6546',
			'7294',
			'7573',
			'8032',
			'9961',
			'11182',
			'11613',
			'5649',
			'11406',
			'11407'
		]);
	});

	it('carries on from the earliest unlisted member when the links run out', () => {
		const e = (id: string, start: string) => ({ ...entryOf(id), start });
		const r: MockRun = {
			...run('bleach'),
			nameId: 'a',
			entries: [e('a', '2001'), e('b', '2002'), e('c', '2003'), e('d', '2004')],
			links: [
				['b', 'c'],
				['c', 'b'],
				['b', 'd'],
				['d', 'b']
			],
			progress: {}
		};
		expect(ids(sequence(r, 'linked'))).toEqual(['a', 'b', 'c', 'd']);
	});
});

function entryOf(id: string): MockEntry {
	return {
		id,
		title: id,
		subtype: 'TV',
		start: null,
		episodes: 12,
		status: 'finished',
		poster: null,
		cover: null
	};
}

describe('rail items', () => {
	it('opens each Fate branch with its way and a divider', () => {
		const items = railItems(run('fate'), 'linked');
		expect(items.map((i) => [i.entry.id, i.key?.kind ?? null, i.divider])).toEqual([
			['6028', null, false],
			['7658', null, false],
			['323', 'way', true],
			['9718', 'way', true],
			['7882', null, false],
			['10052', null, false]
		]);
		expect(items[3].key).toEqual({ kind: 'way', n: 2, count: 2 });
	});

	it('marks the Semi-Final as leading into The Final, unnumbered', () => {
		const semi = railItems(run('gintama'), 'linked').find((i) => i.entry.id === '43829')!;
		expect(semi).toMatchObject({
			position: null,
			divider: true,
			key: { kind: 'leads-into', entry: { id: '43683' }, position: 10 }
		});
	});

	it('opens the Monogatari loop with its size, and divides the line after it', () => {
		const items = railItems(run('monogatari'), 'linked');
		expect(items[1]).toMatchObject({
			key: { kind: 'loop', count: 10 },
			divider: true,
			inLoop: true
		});
		expect(items.filter((i) => i.inLoop)).toHaveLength(10);
		expect(items[11].divider).toBe(true);
		expect(items[12].divider).toBe(false);
	});

	it('is a numbered line with no keys in release order', () => {
		const items = railItems(run('gintama'), 'released');
		expect(items.every((i) => i.key === null && !i.divider)).toBe(true);
		expect(items.map((i) => i.position)).toEqual(items.map((_, i) => i + 1));
	});
});

describe('loop size', () => {
	it('counts the loop an entry sits in, and nothing outside one', () => {
		expect(loopSize(run('monogatari'), '6546', 'linked')).toBe(10);
		expect(loopSize(run('monogatari'), '3919', 'linked')).toBe(0);
	});
});
