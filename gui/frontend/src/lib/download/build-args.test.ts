import { describe, expect, it } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';
import { buildDownloadArgs } from './build-args';

// Codex P2 #3243357083: the detail page was conflating Kitsu's
// announced total with allmanga's currently-released count when
// composing DownloadArgs. The backend picker now compares `expected`
// against allmanga's planned `episodeCount` (b629127), so passing
// the released-so-far number here drops the real candidate by
// planned-count divergence. Extracted to a pure helper so both the
// detail page and the play page share one (correct) path.
function refWithCount(episode_count: number | null): KitsuAnimeRef {
	return {
		id: 'k1',
		canonical_title: 'Some Show',
		titles: { en_jp: 'Some Show JP' },
		slug: null,
		synopsis: null,
		start_date: '2026-01-15',
		end_date: null,
		episode_count,
		average_rating: null,
		subtype: null,
		status: null,
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

describe('buildDownloadArgs', () => {
	it("uses Kitsu's announced episode_count, not the allmanga released count", () => {
		const args = buildDownloadArgs({
			detail: refWithCount(12),
			episode: 1,
			mode: 'sub',
			quality: 'best',
			kitsuId: 'k1'
		});
		expect(args.episode_count).toBe(12);
	});

	it('omits episode_count when Kitsu has not indexed one', () => {
		const args = buildDownloadArgs({
			detail: refWithCount(null),
			episode: 1,
			mode: 'sub',
			quality: 'best',
			kitsuId: 'k1'
		});
		expect(args.episode_count).toBeUndefined();
	});

	it('threads canonical title, year, alt titles, and kitsu_id', () => {
		const args = buildDownloadArgs({
			detail: refWithCount(12),
			episode: 5,
			mode: 'dub',
			quality: '1080',
			kitsuId: 'k1'
		});
		expect(args.title).toBe('Some Show');
		expect(args.episode).toBe('5');
		expect(args.mode).toBe('dub');
		expect(args.quality).toBe('1080');
		expect(args.year).toBe(2026);
		expect(args.alt_titles).toEqual(['Some Show JP']);
		expect(args.kitsu_id).toBe('k1');
	});
});
