import { describe, expect, it } from 'vitest';
import { MOCK_RUNS } from './fixtures';
import type { MockRun } from './types';
import { asRef, franchiseName, entryIn, runFor } from './view';

const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;

describe('mockup view', () => {
	it('finds the run of a member, and none for an outsider', () => {
		expect(runFor('46598')?.key).toBe('jojo');
		expect(runFor('610')).toBeNull();
		expect(entryIn(run('jojo'), '610')?.subtype).toBe('OVA');
		expect(entryIn(run('jojo'), 'nope')).toBeNull();
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

	it('names a franchise without the format suffix', () => {
		expect(franchiseName('JoJo no Kimyou na Bouken (TV)')).toBe('JoJo no Kimyou na Bouken');
		expect(franchiseName('Fate/Zero')).toBe('Fate/Zero');
	});
});
