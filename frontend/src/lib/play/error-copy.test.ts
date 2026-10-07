import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import {
	describeError,
	describeExternalLaunchFailure,
	describePlayFailure,
	describeRateLimit,
	describeSourceDown
} from './error-copy';

describe('describeRateLimit', () => {
	it('maps the typed rate limit with its wait for reuse by route-local mappers', () => {
		// The detail and home pages keep their own surface-specific
		// mappers for the older kinds; this shared first-chance helper
		// is what lets all surfaces render the busy-source copy (and
		// the upstream's advertised wait) without duplicating it.
		const msg = describeRateLimit({ kind: 'rate_limited', retry_after_secs: 7 });
		expect(msg).toMatch(/busy/i);
		expect(msg).toMatch(/7/);
	});

	it('returns null for every other error so surface copy stays local', () => {
		expect(describeRateLimit({ kind: 'timeout' })).toBeNull();
		expect(describeRateLimit({ kind: 'network' })).toBeNull();
		expect(describeRateLimit(new Error('x'))).toBeNull();
		expect(describeRateLimit(null)).toBeNull();
	});
});

describe('describeError', () => {
	// User-facing copy: a localized sentence chosen by the error's kind.
	// `detail` is the backend's free text for logs (ParseFailed carries
	// serde's message, a URL that failed validation, …) and must never
	// reach the screen, nor may a raw `kind` token or a thrown Error's
	// message.
	const KINDS = [
		'parse_failed',
		'metadata',
		'network',
		'gate_refused',
		'upstream',
		'http',
		'timeout',
		'rate_limited',
		'cache',
		'io',
		'config',
		'no_results',
		'scraper',
		'invalid_token',
		'something_new'
	];

	it('never prints a ParseFailed detail', () => {
		const detail = 'expected value at line 1 column 1';
		const msg = describeError({ kind: 'parse_failed', detail });
		expect(msg).not.toContain(detail);
		expect(msg).not.toContain('parse_failed');
		expect(msg).toMatch(/couldn't read/i);
	});

	it('the detail never changes the copy, whatever the kind', () => {
		fc.assert(
			fc.property(fc.constantFrom(...KINDS), fc.string(), (kind, detail) => {
				expect(describeError({ kind, detail })).toBe(describeError({ kind }));
			})
		);
	});

	it('never shows the raw kind token', () => {
		for (const kind of KINDS) {
			expect(describeError({ kind }), kind).not.toContain(kind);
		}
	});

	it('names the cause by kind', () => {
		expect(describeError({ kind: 'network' })).toMatch(/check your connection/i);
		expect(describeError({ kind: 'gate_refused' })).toBe(describeError({ kind: 'network' }));
	});

	it('does not tell the user to check their connection when the service did answer', () => {
		// `upstream` is a non-success status from a service that was
		// reached, and `http` a non-JSON error body from the local
		// backend; neither is a connection problem, and for a 5xx the
		// play copy already says nothing is wrong on the user's end.
		const answered = describeError({ kind: 'upstream', status: 503 });
		expect(answered).toMatch(/answered with an error/i);
		expect(answered).not.toBe(describeError({ kind: 'network' }));
		expect(describeError({ kind: 'upstream', status: 404 })).toBe(answered);
		expect(describeError({ kind: 'http', status: 500 })).toBe(answered);
		expect(describeError({ kind: 'timeout' })).toMatch(/took too long/i);
		expect(describeError({ kind: 'rate_limited' })).toMatch(/busy/i);
		expect(describeError({ kind: 'metadata' })).toBe(describeError({ kind: 'parse_failed' }));
		expect(describeError({ kind: 'cache' })).toMatch(/this computer/i);
		expect(describeError({ kind: 'io' })).toBe(describeError({ kind: 'cache' }));
		expect(describeError({ kind: 'config' })).toBe(describeError({ kind: 'cache' }));
	});

	it('falls back to generic copy for anything else, without echoing it', () => {
		const generic = describeError({ kind: 'something_new' });
		expect(generic).toMatch(/something went wrong/i);
		expect(describeError(new Error('boom'))).toBe(generic);
		expect(describeError('plain string')).toBe(generic);
		expect(describeError(42)).toBe(generic);
		expect(describeError(null)).toBe(generic);
		expect(describeError(undefined)).toBe(generic);
		expect(describeError({ kind: 1, detail: 'x' })).toBe(generic);
	});
});

describe('describePlayFailure', () => {
	it('matches the no_results branch', () => {
		expect(describePlayFailure({ kind: 'scraper', detail: 'no_results' })).toMatch(
			/Couldn't find this title/
		);
	});

	it('names the episode, not the title, when the show was found and the episode was not', () => {
		// The show is in the catalogue; this episode has no stream in
		// the requested audio. Telling the user the title is not in the
		// catalogue is wrong on both counts.
		const msg = describePlayFailure({ kind: 'episode_unavailable' });
		expect(msg).toMatch(/episode/i);
		expect(msg, 'not the generic shrug').not.toBe(describePlayFailure({ kind: 'io' }));
		expect(msg, 'not the catalogue miss').not.toBe(
			describePlayFailure({ kind: 'scraper', detail: 'no_results' })
		);
		expect(
			describePlayFailure({ kind: 'episode_unavailable' }, { noResults: () => 'NOT IN CATALOGUE' })
		).toBe(msg);
	});

	it('matches the scraper branch when no_results is not present', () => {
		expect(describePlayFailure({ kind: 'scraper', detail: 'allmanga 503' })).toMatch(
			/streaming source looks unhappy/
		);
	});

	it('matches the timeout branch', () => {
		expect(describePlayFailure({ kind: 'timeout' })).toMatch(/took too long to respond/);
	});

	it('matches the network branch on either kind', () => {
		expect(describePlayFailure({ kind: 'network' })).toMatch(/Network trouble/);
		expect(describePlayFailure({ kind: 'upstream', detail: '503' })).toMatch(/Network trouble/);
	});

	it('surfaces the rate-limit wait when the backend carries one', () => {
		// The provider answers its throttle with an explicit "try again in
		// N seconds" — the backend forwards it as retry_after_secs and
		// the copy must pass the number on instead of the generic
		// "couldn't start" shrug.
		const msg = describePlayFailure({ kind: 'rate_limited', retry_after_secs: 9 });
		expect(msg).toMatch(/busy/i);
		expect(msg).toMatch(/9/);
	});

	it('still names the busy source when the rate limit has no hint', () => {
		expect(describePlayFailure({ kind: 'rate_limited', retry_after_secs: null })).toMatch(/busy/i);
	});

	it('falls back to a generic retry message for unrecognized errors', () => {
		// A plain Error or an unexpected shape lands here. The copy
		// stays optimistic — "try again" — because the most common
		// real-world cause is a transient hiccup the user hasn't
		// seen before.
		expect(describePlayFailure(new Error('something weird'))).toMatch(
			/Couldn't start this episode right now/
		);
		expect(describePlayFailure({ unexpected: true })).toMatch(
			/Couldn't start this episode right now/
		);
	});

	it('treats no_results case-insensitively (backend may shift casing)', () => {
		// The classifier lowercases before matching, so an upstream
		// that emits "NO_RESULTS" still hits the catalogue-miss
		// branch.
		expect(describePlayFailure({ kind: 'NO_RESULTS' })).toMatch(/Couldn't find this title/);
	});
});

describe('describeExternalLaunchFailure', () => {
	it('names the configured binary when the spawn failed', () => {
		// The user's specific complaint: the small inline notice was
		// easy to miss. The modal-driven copy still has to surface
		// *which* command went missing so the user can fix it in
		// settings without guessing. The backend ships the binary in
		// the typed payload.
		const msg = describeExternalLaunchFailure({
			kind: 'player_spawn_failed',
			binary: 'mpv'
		});
		expect(msg).toMatch(/mpv/);
	});

	it('includes a hint about PATH / settings for the spawn-failed case', () => {
		// The body needs to point the user at *what to do next* —
		// install the player or pick its full path in settings —
		// otherwise the modal just relabels the failure without
		// helping. Pin a stable substring so a copy refresh that
		// drops the actionable hint gets caught.
		const msg = describeExternalLaunchFailure({
			kind: 'player_spawn_failed',
			binary: 'vlc'
		});
		expect(msg).toMatch(/PATH|Settings/i);
	});

	it('falls back to describePlayFailure copy for non-spawn errors', () => {
		// External launch resolves the same upstream URL as embedded
		// play, so it can hit the same scraper / network branches.
		// Reusing describePlayFailure keeps the user-facing copy
		// consistent across both surfaces (no debug-y "External
		// player failed: scraper" leak).
		expect(describeExternalLaunchFailure({ kind: 'timeout' })).toMatch(/took too long/);
		expect(describeExternalLaunchFailure({ kind: 'network' })).toMatch(/Network trouble/);
	});

	it('rejects payloads with the wrong shape and falls back to generic copy', () => {
		// Defensive: a backend drift that drops `binary` or sends a
		// non-string shouldn't crash the renderer. The generic
		// "try again" message is the right safety net.
		expect(describeExternalLaunchFailure({ kind: 'player_spawn_failed' })).toMatch(
			/Couldn't start this episode right now/
		);
		expect(describeExternalLaunchFailure({ kind: 'player_spawn_failed', binary: '' })).toMatch(
			/Couldn't start this episode right now/
		);
	});
});

describe('describePlayFailure — the provider being down names itself', () => {
	it('an upstream 5xx blames the source, not the connection', () => {
		// A 503 is the provider explicitly answering "service
		// unavailable" — maintenance or an outage. The old copy said
		// "check your connection", which sent the user chasing their
		// own network and VPN through a provider maintenance window.
		expect(describePlayFailure({ kind: 'upstream', status: 503 })).toBe(
			m.play_play_failure_source_down()
		);
		expect(describePlayFailure({ kind: 'upstream', status: 502 })).toBe(
			m.play_play_failure_source_down()
		);
	});

	it('genuine connection failures keep the check-your-connection copy', () => {
		expect(describePlayFailure({ kind: 'network' })).toBe(m.play_play_failure_network());
		// A 4xx upstream is not the down-for-maintenance shape.
		expect(describePlayFailure({ kind: 'upstream', status: 403 })).toBe(
			m.play_play_failure_network()
		);
	});
});

describe('describeSourceDown', () => {
	it('is the shared first-chance branch every play surface calls', () => {
		// The detail and home pages keep their own failure mappers;
		// only shared first-chance helpers reach all three surfaces —
		// the rate-limit branch already works this way, and the old
		// copy showed on a provider 503 precisely because the
		// source-down branch lived in one mapper of three.
		expect(describeSourceDown({ kind: 'upstream', status: 503 })).toBe(
			m.play_play_failure_source_down()
		);
		expect(describeSourceDown({ kind: 'upstream', status: 500 })).toBe(
			m.play_play_failure_source_down()
		);
		expect(describeSourceDown({ kind: 'upstream', status: 403 })).toBeNull();
		expect(describeSourceDown({ kind: 'network' })).toBeNull();
		expect(describeSourceDown('boom')).toBeNull();
	});
});

describe('describePlayFailure — one mapper for every surface', () => {
	it('a surface may override only the no-results copy', () => {
		// The detail page's "isn't in the catalogue" phrasing is the
		// one deliberate per-surface difference; everything else was
		// triplicated drift (the home page's copies were hardcoded
		// English). One mapper, one override point.
		expect(describePlayFailure({ kind: 'no_results' }, { noResults: () => 'not indexed' })).toBe(
			'not indexed'
		);
		expect(describePlayFailure({ kind: 'no_results' })).toBe(m.play_play_failure_no_results());
		// The override touches nothing else.
		expect(describePlayFailure({ kind: 'timeout' }, { noResults: () => 'not indexed' })).toBe(
			m.play_play_failure_timeout()
		);
	});
});
