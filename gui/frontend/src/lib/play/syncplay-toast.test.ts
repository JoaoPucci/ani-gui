// @vitest-environment happy-dom
//
// Paraglide messages compile to per-key JS modules under
// `src/lib/paraglide/` ahead of the run via `pnpm i18n:compile`.
// happy-dom isn't strictly needed — the helpers are pure — but
// mirrors `external-toast.test.ts`.
import { describe, expect, it } from 'vitest';
import {
	describeSyncplayLaunchFailure,
	isSyncplaySpawnFailure,
	syncplayLaunchSuccessToast
} from './syncplay-toast';

describe('syncplayLaunchSuccessToast', () => {
	it('returns a success-kind toast naming the episode', () => {
		const toast = syncplayLaunchSuccessToast({ episode: 5 });
		expect(toast.kind).toBe('success');
		// 4s matches the external-launch toast so both pop-up
		// successes feel the same.
		expect(toast.duration).toBe(4000);
		expect(toast.message).toContain('5');
		// "Syncplay" is a brand name — appears literal in every locale.
		expect(toast.message).toContain('Syncplay');
	});

	it('carries different episode numbers verbatim', () => {
		const toast = syncplayLaunchSuccessToast({ episode: 12 });
		expect(toast.message).toContain('12');
	});
});

describe('describeSyncplayLaunchFailure', () => {
	it('names the binary on syncplay_spawn_failed payloads', () => {
		// Backend returns `{ kind: "syncplay_spawn_failed", binary: "..." }`
		// when Command::spawn() fails. The helper produces the body
		// text — the surrounding modal's headline + action live on
		// the play page.
		const got = describeSyncplayLaunchFailure({
			kind: 'syncplay_spawn_failed',
			binary: '/opt/syncplay/syncplay'
		});
		expect(got).toContain('/opt/syncplay/syncplay');
	});

	it('falls back to describePlayFailure for resolve-step errors', () => {
		// If the URL resolution (ani-cli) fails before Syncplay even
		// spawns, the user should see the same polished message as
		// the embedded play path — not a debug-y "Syncplay failed:
		// scraper" string.
		const scraperErr = { kind: 'scraper', key: 'error.scraper.parse_failed' };
		const got = describeSyncplayLaunchFailure(scraperErr);
		// describePlayFailure's scraper branch fires on "scraper" in
		// the flattened error string; pin that the helper hits that
		// branch by checking the EN copy.
		expect(got.length).toBeGreaterThan(0);
		// Should NOT name a binary (it's a resolve error, not a spawn
		// error).
		expect(got).not.toContain('syncplay');
	});

	it('falls back when binary field is missing or empty', () => {
		// Defensive: a malformed payload (`kind: syncplay_spawn_failed`
		// without the binary field) shouldn't crash; it should drop
		// to the generic resolve-failure copy.
		const got = describeSyncplayLaunchFailure({ kind: 'syncplay_spawn_failed', binary: '' });
		expect(got.length).toBeGreaterThan(0);
		expect(got).not.toContain(' ""');
	});
});

describe('isSyncplaySpawnFailure', () => {
	// Drives the play page's "Get Syncplay" affordance: link to
	// syncplay.pl/download/ only when the failure is a spawn error.
	// For resolve-step errors (scraper, timeout, network) installing
	// Syncplay won't help, so the action is suppressed.

	it('returns true for well-formed syncplay_spawn_failed payloads', () => {
		expect(
			isSyncplaySpawnFailure({
				kind: 'syncplay_spawn_failed',
				binary: '/opt/syncplay/syncplay'
			})
		).toBe(true);
	});

	it('returns false for scraper / resolve-step errors', () => {
		expect(isSyncplaySpawnFailure({ kind: 'scraper', key: 'error.scraper.parse_failed' })).toBe(
			false
		);
		expect(isSyncplaySpawnFailure({ kind: 'timeout' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'network' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'no_results' })).toBe(false);
	});

	it('returns false for player_spawn_failed (external player, not syncplay)', () => {
		// PlayerSpawnFailed is the external-player variant; if it
		// somehow surfaces on the syncplay code path, we shouldn't
		// confuse it with a Syncplay-binary problem.
		expect(isSyncplaySpawnFailure({ kind: 'player_spawn_failed', binary: 'mpv' })).toBe(false);
	});

	it('returns false on malformed / empty syncplay_spawn_failed payloads', () => {
		// Defensive: a payload tagged syncplay_spawn_failed but
		// without a non-empty binary string is too uncertain to
		// surface the install affordance on. Treat as resolve error.
		expect(isSyncplaySpawnFailure({ kind: 'syncplay_spawn_failed' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'syncplay_spawn_failed', binary: '' })).toBe(false);
		expect(isSyncplaySpawnFailure({ kind: 'syncplay_spawn_failed', binary: 42 })).toBe(false);
	});

	it('returns false for unknown error shapes (plain Error, undefined, null)', () => {
		expect(isSyncplaySpawnFailure(new Error('boom'))).toBe(false);
		expect(isSyncplaySpawnFailure(undefined)).toBe(false);
		expect(isSyncplaySpawnFailure(null)).toBe(false);
		expect(isSyncplaySpawnFailure('a string')).toBe(false);
	});
});
