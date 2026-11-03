import { describe, expect, it } from 'vitest';
import { MOCK_RUNS } from './fixtures';
import type { MockRun } from './types';
import { asRef, franchiseName, scrollLeftFor, entryIn, handoffs, runFor, upNextFor } from './view';

const TODAY = '2026-11-02';
const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;

describe('mockup view', () => {
	it('finds the run of a member, and none for an outsider', () => {
		expect(runFor('46598')?.key).toBe('jojo');
		expect(runFor('610')).toBeNull();
		expect(entryIn(run('jojo'), '610')?.subtype).toBe('OVA');
		expect(entryIn(run('jojo'), 'nope')).toBeNull();
	});

	it('answers the end card in each of its four cases', () => {
		expect(upNextFor(run('bleach'), '43078', 'linked', TODAY)).toMatchObject({
			kind: 'one',
			entry: { id: '46903' }
		});
		expect(upNextFor(run('monogatari'), '48434', 'linked', TODAY)).toMatchObject({
			kind: 'unaired',
			entry: { id: '50013' }
		});
		expect(upNextFor(run('fate'), '7658', 'linked', TODAY).kind).toBe('choices');
		expect(upNextFor(run('bleach'), '49444', 'linked', TODAY).kind).toBe('none');
	});

	it('hands Continue on only where one unstarted entry follows the last watched', () => {
		const got = handoffs('linked', TODAY).map((h) => [h.finished.id, h.next.id]);
		expect(got).toEqual([
			['43078', '46903'],
			['44240', '46038']
		]);
	});
});

describe('mockup adapters', () => {
	it('shapes an entry for PosterCard', () => {
		const e = entryIn(run('jojo'), '610')!;
		const ref = asRef(e);
		expect(ref).toMatchObject({ id: '610', canonical_title: e.title, episode_count: e.episodes });
		expect(ref.poster_image?.medium).toBe(e.poster);
		expect(asRef({ ...e, poster: null, cover: null })).toMatchObject({
			poster_image: null,
			cover_image: null
		});
	});

	it('places an item in a scroller at the share asked, clamped to its range', () => {
		const box = { clientWidth: 400, scrollWidth: 1200 };
		expect(scrollLeftFor(box, 600, 100, 0.5)).toBe(450);
		expect(scrollLeftFor(box, 600, 100, 0.75)).toBe(375);
		expect(scrollLeftFor(box, 50, 100, 0.5)).toBe(0);
		expect(scrollLeftFor(box, 1150, 100, 0.5)).toBe(800);
	});

	it('names a franchise without the format suffix', () => {
		expect(franchiseName('JoJo no Kimyou na Bouken (TV)')).toBe('JoJo no Kimyou na Bouken');
		expect(franchiseName('Fate/Zero')).toBe('Fate/Zero');
	});
});
