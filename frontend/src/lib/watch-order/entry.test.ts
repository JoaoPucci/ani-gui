import { describe, expect, it } from 'vitest';
import { isUnaired, progressOf, progressShare, yearOf } from './entry';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import type { WatchEntry } from './types';

const TODAY = '2026-11-02';
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
function entryIn(r: FixtureRun, id: string): WatchEntry {
	return [...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps].find(
		(e) => e.id === id
	)!;
}

describe('entry readings', () => {
	it('reads progress as none, watched or part-way', () => {
		const jojo = run('jojo');
		const at = (id: string) => progressOf(entryIn(jojo, id), jojo.progress[id]);
		expect(at('610')).toEqual({ kind: 'none' });
		expect(at('7158')).toEqual({ kind: 'watched' });
		expect(at('46598')).toEqual({ kind: 'partway', episode: 13, total: 14 });
		expect(progressShare(at('7158'))).toBe(1);
		expect(progressShare(at('610'))).toBe(0);
		expect(progressShare({ kind: 'partway', episode: 3, total: null })).toBe(0);
		expect(progressShare({ kind: 'partway', episode: 7, total: 14 })).toBe(0.5);
	});

	it('knows aired from unaired', () => {
		const mono = run('monogatari');
		expect(isUnaired(entryIn(mono, '50013'), TODAY)).toBe(true);
		expect(isUnaired(entryIn(mono, '3919'), TODAY)).toBe(false);
		expect(yearOf(entryIn(mono, '3919'))).toBe('2009');
		expect(yearOf(entryIn(mono, '50013'))).toBeNull();
	});
});
