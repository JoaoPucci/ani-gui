/**
 * The /anime/[id] and /play/[id] routes both keep an in-memory
 * Kitsu-episode-page cache so prev/next pagination feels instant.
 * SvelteKit reuses the same component instance when the user
 * navigates between two URLs that match the same dynamic route
 * (e.g. /anime/A → /anime/B via a similar-titles card). The id-
 * change reset that nulls `episodes`, `detail`, etc. used to leave
 * the page-cache untouched, so the new show's first paint would
 * render the previous show's thumbnails for whichever Kitsu pages
 * were already warmed.
 *
 * This module owns the cache instance and the reset call, so the
 * route components route their cache use through it instead of
 * holding a bare `SvelteMap` they have to remember to clear.
 */
import { describe, it, expect } from 'vitest';
import type { KitsuEpisode } from '$lib/api';
import { createEpisodePageCache, resetEpisodePageCache } from './episode-page-cache';

function ep(n: number, label: string): KitsuEpisode {
	return {
		id: `${label}-${n}`,
		canonical_title: `${label} ep ${n}`,
		season_number: 1,
		number: n,
		relative_number: n,
		length: 24,
		synopsis: null,
		airdate: null,
		thumbnail: { original: `https://kitsu/${label}/${n}.jpg` }
	};
}

describe('episode page cache', () => {
	it('starts empty and stores entries by Kitsu page number', () => {
		const cache = createEpisodePageCache();
		expect(cache.size).toBe(0);
		cache.set(1, [ep(1, 'A'), ep(2, 'A')]);
		expect(cache.get(1)?.[0].id).toBe('A-1');
		expect(cache.has(1)).toBe(true);
	});

	it('drops every entry on reset, so a subsequent get for the same key is a miss', () => {
		const cache = createEpisodePageCache();
		cache.set(1, [ep(1, 'A')]);
		cache.set(2, [ep(21, 'A')]);
		expect(cache.size).toBe(2);

		resetEpisodePageCache(cache);

		expect(cache.size).toBe(0);
		expect(cache.has(1)).toBe(false);
		expect(cache.has(2)).toBe(false);
		expect(cache.get(1)).toBeUndefined();
	});

	it('reset on the same instance leaves it usable for the next show', () => {
		const cache = createEpisodePageCache();
		cache.set(1, [ep(1, 'A')]);
		resetEpisodePageCache(cache);
		cache.set(1, [ep(1, 'B')]);
		expect(cache.get(1)?.[0].id).toBe('B-1');
	});
});
