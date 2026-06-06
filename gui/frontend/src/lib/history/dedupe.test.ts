import { describe, expect, it } from 'vitest';
import { dedupeHistoryByKitsuId } from './dedupe';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

function entry(id: string, ep = '1', title = `Show ${id}`): HistoryEntry {
	return { id, ep_no: ep, title };
}

function kitsu(id: string): KitsuAnimeRef {
	return {
		id,
		slug: `slug-${id}`,
		canonical_title: `Kitsu ${id}`,
		titles: {},
		episode_count: null,
		subtype: 'TV',
		status: 'current',
		poster_image: null,
		start_date: null
	} as unknown as KitsuAnimeRef;
}

describe('dedupeHistoryByKitsuId', () => {
	it('returns the input untouched when no two entries share a Kitsu id', () => {
		// The common case — every history row maps to a distinct Kitsu
		// entry. Dedupe must be a strict no-op here so the strip
		// renders exactly what it did before the filter shipped.
		const a = entry('all-1');
		const b = entry('all-2');
		const c = entry('all-3');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-1': kitsu('k-1'),
			'all-2': kitsu('k-2'),
			'all-3': kitsu('k-3')
		};
		expect(dedupeHistoryByKitsuId([a, b, c], matches)).toEqual([a, b, c]);
	});

	it('keeps the FIRST occurrence of each Kitsu id (sort order wins)', () => {
		// The empirical trigger #116 targets: allmanga catalog drift
		// across ani-cli runs produces two hsts rows whose alias-walk
		// both land on the same Kitsu entry. The caller (the page)
		// passes entries in sortByWatchedAt order, so the first
		// occurrence is the most recently watched — that's the one
		// the user expects to click to resume.
		const recent = entry('all-recent');
		const stale = entry('all-stale');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-recent': kitsu('k-shared'),
			'all-stale': kitsu('k-shared')
		};
		expect(dedupeHistoryByKitsuId([recent, stale], matches)).toEqual([recent]);
	});

	it('keeps unresolved entries (match === undefined) — they cannot be deduped yet', () => {
		// The loader writes matches per-row asynchronously. Until a
		// row's match lands, we don't know its Kitsu id, so we have
		// no basis for hiding it. Preserve the per-row release
		// pattern from PR #50: the row stays visible (as a loading
		// card) until the loader finishes for that entry. Even if a
		// sort-later sibling resolves first and claims a Kitsu id,
		// we don't drop the unresolved sibling yet — only the next
		// derived re-run, when its match lands too, makes the call.
		const a = entry('all-1');
		const b = entry('all-2');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-1': kitsu('k-1')
			// all-2 is unresolved
		};
		expect(dedupeHistoryByKitsuId([a, b], matches)).toEqual([a, b]);
	});

	it('keeps entries with a null match (no Kitsu match found)', () => {
		// resolveKitsuMatch returned null — the alias-walk found
		// nothing on Kitsu. Without a Kitsu id we have no equivalence
		// signal, so two null-matched rows might be the same show or
		// might not be. Keep them; the user still wants to see them
		// and can click them (routes to /search per the page's null-
		// match branch).
		const a = entry('all-1');
		const b = entry('all-2');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-1': null,
			'all-2': null
		};
		expect(dedupeHistoryByKitsuId([a, b], matches)).toEqual([a, b]);
	});

	it('handles multiple groups of duplicates in one pass', () => {
		// Mixed scenario: two distinct dupe groups plus a unique row,
		// with one row still unresolved. Each group's first occurrence
		// wins; unresolved and null-match rows pass through.
		const a = entry('all-1');
		const b = entry('all-2');
		const c = entry('all-3');
		const d = entry('all-4');
		const e = entry('all-5');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-1': kitsu('k-A'),
			'all-2': kitsu('k-B'),
			'all-3': kitsu('k-A'),
			// all-4 unresolved
			'all-5': kitsu('k-B')
		};
		expect(dedupeHistoryByKitsuId([a, b, c, d, e], matches)).toEqual([a, b, d]);
	});

	it('returns an empty array when history is empty', () => {
		expect(dedupeHistoryByKitsuId([], {})).toEqual([]);
	});

	it('preserves entry references (no copies)', () => {
		// Downstream code may rely on entry reference identity for
		// Svelte keyed-each diffing. Don't construct new objects.
		const a = entry('all-1');
		const result = dedupeHistoryByKitsuId([a], { 'all-1': kitsu('k-1') });
		expect(result[0]).toBe(a);
	});
});
