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

	it('keeps the highest-progress occurrence of each Kitsu id, ties broken by first', () => {
		// The empirical trigger #116 targets: allmanga catalog drift
		// across ani-cli runs produces two hsts rows whose alias-walk
		// both land on the same Kitsu entry. The user's true "where
		// am I?" signal is ep_no — pick the most-advanced row so the
		// surviving card resumes from their actual progress. Ties on
		// ep_no fall back to input order (most-recent first under
		// sortByWatchedAt), which matches the original first-wins
		// intuition for the common no-drift case.
		const lo = entry('all-lo', '3');
		const hi = entry('all-hi', '12');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-lo': kitsu('k-shared'),
			'all-hi': kitsu('k-shared')
		};
		// Order in input doesn't matter; the row with the higher ep_no
		// wins regardless of position.
		expect(dedupeHistoryByKitsuId([lo, hi], matches)).toEqual([hi]);
		expect(dedupeHistoryByKitsuId([hi, lo], matches)).toEqual([hi]);
	});

	it('preserves CLI progress over an older GUI-stamped row when ep_no is higher', () => {
		// Codex P2 #3367725631 — the regression my first cut hit. User
		// watched via GUI at ep 5 long ago (stamped, sorted to the
		// top). Allmanga drifted, they continued via ani-cli to ep 12
		// (unstamped, sorted below). Previous "first occurrence wins"
		// dropped the CLI row and the strip would resume from the
		// stale ep 5. The fix: ep_no comparison wins.
		const stampedOld = entry('all-stamped-old', '5');
		const cliCurrent = entry('all-cli-current', '12');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-stamped-old': kitsu('k-shared'),
			'all-cli-current': kitsu('k-shared')
		};
		// sortByWatchedAt would land the stamped row first; the
		// dedupe still picks the row with the actual current progress.
		expect(dedupeHistoryByKitsuId([stampedOld, cliCurrent], matches)).toEqual([cliCurrent]);
	});

	it('falls back to input order when ep_no values are equal', () => {
		// Two history rows for the same show with identical progress
		// (e.g., user paused, drift renamed the show, opened it again
		// before watching anything new). Either could be displayed;
		// pick the sort-earlier row as a deterministic tie-break so
		// the test of behavior is stable across runs.
		const first = entry('all-first', '7');
		const second = entry('all-second', '7');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-first': kitsu('k-shared'),
			'all-second': kitsu('k-shared')
		};
		expect(dedupeHistoryByKitsuId([first, second], matches)).toEqual([first]);
	});

	it('treats malformed ep_no as the lowest possible progress', () => {
		// A user-edited or otherwise broken ep_no (`'abc'`, empty) on
		// one row shouldn't beat a numeric row. Falling back to a
		// low sentinel keeps a real numeric row in the lead.
		const broken = entry('all-broken', 'abc');
		const good = entry('all-good', '4');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-broken': kitsu('k-shared'),
			'all-good': kitsu('k-shared')
		};
		expect(dedupeHistoryByKitsuId([broken, good], matches)).toEqual([good]);
		expect(dedupeHistoryByKitsuId([good, broken], matches)).toEqual([good]);
	});

	it('emits the winner at the position of its group is first encountered', () => {
		// Position-preservation rule: when the winner row appears later
		// in the input than its losing sibling, the surviving card
		// renders at the LOSER's position — so the strip's overall
		// ordering still tracks the sort the page passed in.
		const stampedOld = entry('all-stamped-old', '5');
		const other = entry('all-other', '1');
		const cliCurrent = entry('all-cli-current', '12');
		const matches: Record<string, KitsuAnimeRef | null> = {
			'all-stamped-old': kitsu('k-shared'),
			'all-other': kitsu('k-other'),
			'all-cli-current': kitsu('k-shared')
		};
		// cliCurrent is the dedupe winner for k-shared; it should
		// emit at index 0 (where stampedOld would have been) so the
		// "other" row stays at index 1 — same overall strip order.
		expect(dedupeHistoryByKitsuId([stampedOld, other, cliCurrent], matches)).toEqual([
			cliCurrent,
			other
		]);
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
