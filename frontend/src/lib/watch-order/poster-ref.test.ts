import { describe, expect, it } from 'vitest';
import { FIXTURE_RUNS } from './fixtures/runs';
import { asRef } from './poster-ref';

const ova = FIXTURE_RUNS.find((r) => r.key === 'jojo')!.versions.find((e) => e.id === '610')!;

describe('asRef', () => {
	it('shapes an entry for PosterCard', () => {
		const ref = asRef(ova);
		expect(ref).toMatchObject({
			id: '610',
			canonical_title: ova.title,
			episode_count: ova.episodes,
			start_date: ova.start,
			subtype: ova.subtype
		});
		expect(ref.poster_image?.medium).toBe(ova.poster);
		expect(ref.cover_image?.large).toBe(ova.cover);
	});

	it('has no images where the entry has none', () => {
		expect(asRef({ ...ova, poster: null, cover: null })).toMatchObject({
			poster_image: null,
			cover_image: null
		});
	});
});
