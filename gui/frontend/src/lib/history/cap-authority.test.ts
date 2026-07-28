import { describe, expect, it } from 'vitest';
import { createCapAuthority } from './cap-authority';

describe('createCapAuthority', () => {
	it('passes a loader count through untouched when no click has refined the row', () => {
		const a = createCapAuthority();
		expect(a.resolveLoaderCount('h1', 24)).toBe(24);
		expect(a.resolveLoaderCount('h1', null)).toBeNull();
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
		a.recordClickCap('h1', 12);
		expect(a.resolveLoaderCount('h1', 25)).toBe(12);
	});

	it('keeps the pinned cap when the late loader callback carries no count', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12);
		expect(a.resolveLoaderCount('h1', null)).toBe(12);
	});

	it('lets a newer click supersede an earlier pinned cap', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12);
		a.recordClickCap('h1', 13);
		expect(a.resolveLoaderCount('h1', 25)).toBe(13);
	});

	it('scopes authority per entry', () => {
		const a = createCapAuthority();
		a.recordClickCap('h1', 12);
		expect(a.resolveLoaderCount('h2', 25)).toBe(25);
	});
});
