import { describe, expect, it } from 'vitest';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import { metaLine, numeral, railKeyText, splitMeta } from './meta';
import type { WatchEntry, WatchRun } from './types';

const TODAY = '2026-11-02';
function entryIn(r: WatchRun, id: string): WatchEntry | undefined {
	return [...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps].find(
		(e) => e.id === id
	);
}
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
const line = (key: string, id: string, today = TODAY) =>
	metaLine(entryIn(run(key), id)!, run(key).progress[id], today).map((s) => `${s.kind}:${s.text}`);

describe('meta line', () => {
	it('shows format, year and episode count for an unstarted entry', () => {
		expect(line('bleach', '48015')).toEqual(['word:TV', 'num:2024', 'num:14', 'unit:ep']);
	});

	it('shows progress or watched in place of the count', () => {
		expect(line('jojo', '46598')).toEqual(['word:TV', 'num:2022', 'num:EP 13 of 14']);
		expect(line('jojo', '7158')).toEqual(['word:TV', 'num:2012', 'watched:Watched']);
	});

	it('leaves the count off a single video', () => {
		expect(line('gintama', '43683')).toEqual(['word:MOVIE', 'num:2021']);
	});

	it('dates an unaired entry, or says no date is announced', () => {
		expect(line('monogatari', '50013')).toEqual(['word:SPECIAL', 'word:Date not announced']);
		expect(line('bleach', '49444', '2026-01-01')).toEqual(['word:TV', 'word:Coming July 2026']);
	});

	it('reads progress without a total as the episode alone', () => {
		const r = { ...run('jojo'), progress: { '49847': 3 } };
		const e = { ...entryIn(r, '49847')!, episodes: null };
		expect(metaLine(e, r.progress['49847'], TODAY).at(-1)).toEqual({ kind: 'num', text: 'EP 3' });
	});

	it('pads positions to two digits', () => {
		expect(numeral(5)).toBe('05');
		expect(numeral(16)).toBe('16');
	});
});

describe('rail key text', () => {
	const e = entryIn(run('gintama'), '43683')!;
	const label = (x: { title: string }) => x.title;
	it('says where the viewer is first', () => {
		expect(railKeyText({ kind: 'loop', count: 10 }, true, label)).toBe('You are here');
	});
	it('names a way, a loop, or the entry a side entry leads into', () => {
		expect(railKeyText({ kind: 'way', n: 2, count: 2 }, false, label)).toBe('Way 2 of 2');
		expect(railKeyText({ kind: 'loop', count: 10 }, false, label)).toBe(
			'Part of a loop of 10 entries'
		);
		expect(railKeyText({ kind: 'leads-into', entry: e, position: 10 }, false, label)).toBe(
			'Leads into 10'
		);
		expect(railKeyText({ kind: 'leads-into', entry: e, position: null }, false, label)).toBe(
			'Leads into Gintama: The Final'
		);
		expect(railKeyText(null, false, label)).toBeNull();
	});
});

describe('splitMeta', () => {
	const parts = (key: string, id: string, today = TODAY) =>
		splitMeta(metaLine(entryIn(run(key), id)!, run(key).progress[id], today)).map((p) =>
			p.map((s) => s.text)
		);

	it('puts the format and year first, and the rest on its own line', () => {
		expect(parts('jojo', '46598')).toEqual([['TV', '2022'], ['EP 13 of 14']]);
		expect(parts('jojo', '7158')).toEqual([['TV', '2012'], ['Watched']]);
		expect(parts('bleach', '48015')).toEqual([
			['TV', '2024'],
			['14', 'ep']
		]);
	});

	it('keeps a line with nothing after the year whole', () => {
		expect(parts('gintama', '43683')).toEqual([['MOVIE', '2021'], []]);
	});

	it("puts an unaired entry's date on the second line", () => {
		expect(parts('monogatari', '50013')).toEqual([['SPECIAL'], ['Date not announced']]);
	});
});
