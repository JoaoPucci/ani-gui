import { describe, expect, it } from 'vitest';
import { createCapAuthority } from './cap-authority';

describe('createCapAuthority', () => {
	it('passes a loader cap through untouched when no click has refined the row', () => {
		const a = createCapAuthority();
		expect(a.resolveLoaderCap('h1', { count: 24, approximate: false })).toEqual({
			count: 24,
			approximate: false
		});
		expect(a.resolveLoaderCap('h1', { count: null, approximate: false })).toEqual({
			count: null,
			approximate: false
		});
	});

	it('pins the click-time cap against a later background probe result', () => {
		// Codex P2 #3665110124 — a cold-cache card clicked while its
		// paced background probe is still queued gets an EXACT cap from
		// the interactive lookup; the background probe landing later
		// must not overwrite it. The backend may answer a
		// breaker-refused background detail fetch with an approximate
		// count (the known +1 half-episode case), and the next click
		// would then treat that as authoritative and skip its own
		// lookup — selecting a phantom episode.
		const a = createCapAuthority();
		a.recordClickCap('h1', 12, false);
		expect(a.resolveLoaderCap('h1', { count: 25, approximate: false })).toEqual({
			count: 12,
			approximate: false
		});
	});

	it('keeps the pinned cap when the late loader callback carries no count', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12, false);
		expect(a.resolveLoaderCap('h1', { count: null, approximate: false }).count).toBe(12);
	});

	it('lets a newer click supersede an earlier pinned cap', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12, false);
		a.recordClickCap('h1', 13, false);
		expect(a.resolveLoaderCap('h1', { count: 25, approximate: false }).count).toBe(13);
	});

	it('scopes authority per entry', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12, false);
		expect(a.resolveLoaderCap('h2', { count: 25, approximate: false }).count).toBe(25);
	});
});

describe('createCapAuthority — count and provenance move together', () => {
	// Codex P2 #3666727714. The count was pinned but the flag was not,
	// so the two could come from different answers: an exact click of
	// 12 followed by an approximate loader result of 13 produced
	// {count: 12, approximate: true}. The next click then revalidates
	// the safe pin, and a detail fetch that falls back to the
	// approximate search count replaces 12 with 13 — a phantom episode
	// manufactured out of a pin that existed to prevent exactly that.
	it('keeps an exact pin exact when an approximate loader result lands', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12, false);
		expect(a.resolveLoaderCap('h1', { count: 13, approximate: true })).toEqual({
			count: 12,
			approximate: false
		});
	});

	it('carries the provenance of the click itself when that lookup was approximate', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 13, true);
		expect(a.resolveLoaderCap('h1', { count: 14, approximate: true })).toEqual({
			count: 13,
			approximate: true
		});
	});

	it('lets an exact loader answer supersede an approximate pin', () => {
		// Freshness is the tie-break between two answers of equal
		// confidence, not a reason to keep the weaker one: a confirmed
		// detail fetch is strictly better information than the search
		// count the click had to settle for.
		const a = createCapAuthority();
		a.recordClickCap('h1', 13, true);
		expect(a.resolveLoaderCap('h1', { count: 12, approximate: false })).toEqual({
			count: 12,
			approximate: false
		});
	});

	it('keeps an approximate pin when the exact loader answer has no count', () => {
		// "Exact" with nothing to be exact about is not an answer.
		const a = createCapAuthority();
		a.recordClickCap('h1', 13, true);
		expect(a.resolveLoaderCap('h1', { count: null, approximate: false })).toEqual({
			count: 13,
			approximate: true
		});
	});
});
