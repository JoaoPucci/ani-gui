import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import {
	describeError,
	describePlayFailure,
	describeRateLimit,
	describeSourceDown
} from './error-copy';
import { describeExternalLaunchFailure } from './external-launch-copy';

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
	// User-facing copy: a localized sentence chosen by the error's kind
	// when the envelope carries no key (key-first resolution has its
	// own cases in describe-error.test.ts).
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
		// backend; neither is a connection problem.
		const answered = describeError({ kind: 'upstream', status: 403 });
		expect(answered).toMatch(/answered with an error/i);
		expect(answered).not.toBe(describeError({ kind: 'network' }));
		expect(describeError({ kind: 'http', status: 500 })).toBe(answered);
	});

	it('tells apart the upstream statuses the user would act on differently', () => {
		// The status is a variant field the copy can read: a missing
		// resource is not worth retrying, a 5xx is the service's own
		// outage, and a 429 is a busy service — three different next
		// steps that one "answered with an error" sentence hid.
		const answered = describeError({ kind: 'upstream', status: 403 });
		const missing = describeError({ kind: 'upstream', status: 404 });
		const down = describeError({ kind: 'upstream', status: 503 });
		expect(missing).toMatch(/couldn't find/i);
		expect(describeError({ kind: 'upstream', status: 410 })).toBe(missing);
		expect(down).toMatch(/down/i);
		expect(describeError({ kind: 'upstream', status: 502 })).toBe(down);
		expect(describeError({ kind: 'upstream', status: 429 })).toBe(
			describeError({ kind: 'rate_limited' })
		);
		expect(new Set([answered, missing, down]).size).toBe(3);
		expect(describeError({ kind: 'upstream' })).toBe(answered);
	});

	it('names a catalogue miss as not found, not as a generic failure', () => {
		expect(describeError({ kind: 'no_results' })).toBe(
			describeError({ kind: 'upstream', status: 404 })
		);
	});

	it("passes on the service's own wait when a rate limit carries one", () => {
		const msg = describeError({ kind: 'rate_limited', retry_after_secs: 9 });
		expect(msg).toMatch(/busy/i);
		expect(msg).toMatch(/9/);
		expect(describeError({ kind: 'rate_limited', retry_after_secs: null })).toBe(
			describeError({ kind: 'rate_limited' })
		);
	});

	it('names the cause by kind (continued)', () => {
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

describe('describeRateLimit — an upstream 429 is a rate limit too', () => {
	it('gives the busy-source copy for { kind: upstream, status: 429 }', () => {
		// The backend keeps an upstream that throttles with HTTP 429 as
		// `upstream` + status, distinct from the in-band `rate_limited`.
		// On the play surfaces it fell through to "Network trouble …
		// check your connection", which sends the user after a problem
		// they do not have.
		expect(describeRateLimit({ kind: 'upstream', status: 429 })).toBe(
			m.play_play_failure_rate_limited()
		);
		expect(describePlayFailure({ kind: 'upstream', status: 429 }, { otherAudio: null })).toBe(
			m.play_play_failure_rate_limited()
		);
		expect(describeRateLimit({ kind: 'upstream', status: 403 })).toBeNull();
	});
});

describe('describePlayFailure', () => {
	it('matches the no_results branch', () => {
		expect(
			describePlayFailure({ kind: 'scraper', detail: 'no_results' }, { otherAudio: null })
		).toMatch(/Couldn't find this title/);
	});

	it('names the episode, not the title, when the show was found and the episode was not', () => {
		// The show is in the catalogue; this episode has no stream in
		// the requested audio. Telling the user the title is not in the
		// catalogue is wrong on both counts.
		const msg = describePlayFailure({ kind: 'episode_unavailable' }, { otherAudio: null });
		expect(msg).toMatch(/episode/i);
		expect(msg, 'not the generic shrug').not.toBe(
			describePlayFailure({ kind: 'io' }, { otherAudio: null })
		);
		expect(msg, 'not the catalogue miss').not.toBe(
			describePlayFailure({ kind: 'scraper', detail: 'no_results' }, { otherAudio: null })
		);
		expect(
			describePlayFailure(
				{ kind: 'episode_unavailable' },
				{ otherAudio: null, noResults: () => 'NOT IN CATALOGUE' }
			)
		).toBe(msg);
	});

	it('matches the scraper branch when no_results is not present', () => {
		expect(
			describePlayFailure({ kind: 'scraper', detail: 'allmanga 503' }, { otherAudio: null })
		).toMatch(/streaming source looks unhappy/);
	});

	it('matches the timeout branch', () => {
		expect(describePlayFailure({ kind: 'timeout' }, { otherAudio: null })).toMatch(
			/took too long to respond/
		);
	});

	it('matches the network branch for a failure to connect', () => {
		expect(describePlayFailure({ kind: 'network' }, { otherAudio: null })).toMatch(
			/Network trouble/
		);
		expect(describePlayFailure({ kind: 'gate_refused' }, { otherAudio: null })).toMatch(
			/Network trouble/
		);
	});

	it('reads an upstream status instead of blaming the connection', () => {
		// `upstream` is the source answering with a non-success status,
		// so the connection is fine. 404/410 is the source not having
		// the thing, which is the catalogue-miss copy; any other status
		// outside the busy (429) and down (5xx) shapes, and an upstream
		// with no status at all, says the source answered with an error.
		const miss = describePlayFailure({ kind: 'upstream', status: 404 }, { otherAudio: null });
		expect(miss).toBe(m.play_play_failure_no_results());
		expect(describePlayFailure({ kind: 'upstream', status: 410 }, { otherAudio: null })).toBe(miss);
		// The detail page's definitive catalogue-miss override is for a
		// title the catalogue lacks, which a 404 does not prove.
		expect(
			describePlayFailure(
				{ kind: 'upstream', status: 404 },
				{ otherAudio: null, noResults: () => 'X' }
			)
		).toBe(miss);
		const answered = describePlayFailure({ kind: 'upstream', status: 403 }, { otherAudio: null });
		expect(answered).toBe(m.play_play_failure_source_error());
		expect(answered).not.toBe(m.play_play_failure_network());
		expect(describePlayFailure({ kind: 'upstream', status: 400 }, { otherAudio: null })).toBe(
			answered
		);
		expect(describePlayFailure({ kind: 'upstream', detail: '503' }, { otherAudio: null })).toBe(
			answered
		);
		expect(describeExternalLaunchFailure({ kind: 'upstream', status: 403 }, null)).toBe(answered);
	});

	it('surfaces the rate-limit wait when the backend carries one', () => {
		// The provider answers its throttle with an explicit "try again in
		// N seconds" — the backend forwards it as retry_after_secs and
		// the copy must pass the number on instead of the generic
		// "couldn't start" shrug.
		const msg = describePlayFailure(
			{ kind: 'rate_limited', retry_after_secs: 9 },
			{ otherAudio: null }
		);
		expect(msg).toMatch(/busy/i);
		expect(msg).toMatch(/9/);
	});

	it('still names the busy source when the rate limit has no hint', () => {
		expect(
			describePlayFailure({ kind: 'rate_limited', retry_after_secs: null }, { otherAudio: null })
		).toMatch(/busy/i);
	});

	it('falls back to a generic retry message for unrecognized errors', () => {
		// A plain Error or an unexpected shape lands here. The copy
		// stays optimistic — "try again" — because the most common
		// real-world cause is a transient hiccup the user hasn't
		// seen before.
		expect(describePlayFailure(new Error('something weird'), { otherAudio: null })).toMatch(
			/Couldn't start this episode right now/
		);
		expect(describePlayFailure({ unexpected: true }, { otherAudio: null })).toMatch(
			/Couldn't start this episode right now/
		);
	});

	it('treats no_results case-insensitively (backend may shift casing)', () => {
		// The classifier lowercases before matching, so an upstream
		// that emits "NO_RESULTS" still hits the catalogue-miss
		// branch.
		expect(describePlayFailure({ kind: 'NO_RESULTS' }, { otherAudio: null })).toMatch(
			/Couldn't find this title/
		);
	});
});

describe('describePlayFailure — the provider being down names itself', () => {
	it('an upstream 5xx blames the source, not the connection', () => {
		// A 503 is the provider explicitly answering "service
		// unavailable" — maintenance or an outage. The old copy said
		// "check your connection", which sent the user chasing their
		// own network and VPN through a provider maintenance window.
		expect(describePlayFailure({ kind: 'upstream', status: 503 }, { otherAudio: null })).toBe(
			m.play_play_failure_source_down()
		);
		expect(describePlayFailure({ kind: 'upstream', status: 502 }, { otherAudio: null })).toBe(
			m.play_play_failure_source_down()
		);
	});

	it('genuine connection failures keep the check-your-connection copy', () => {
		expect(describePlayFailure({ kind: 'network' }, { otherAudio: null })).toBe(
			m.play_play_failure_network()
		);
		// A 4xx upstream is not the down-for-maintenance shape, and not
		// a connection failure either.
		expect(describePlayFailure({ kind: 'upstream', status: 403 }, { otherAudio: null })).not.toBe(
			m.play_play_failure_source_down()
		);
		expect(describePlayFailure({ kind: 'upstream', status: 403 }, { otherAudio: null })).not.toBe(
			m.play_play_failure_network()
		);
	});
});

describe('describePlayFailure — a stream server that did not serve', () => {
	it('names the source’s video server and the episode, not the user’s connection', () => {
		// The backend's verdict for an episode chain that did not serve
		// — the embed host unreachable or timing out, a page or payload
		// or playlist it could not play — past searches that answered.
		// It carries the stage and host for the log; the copy names
		// neither. Shown as the transport failure it stands for, it
		// sent the user after a connection the searches got through on.
		const e = {
			kind: 'stream_server_failed',
			key: 'error.play.stream_server_failed',
			stage: 'embed',
			host: 'zokoanime.video'
		};
		const msg = describePlayFailure(e, { otherAudio: null });
		expect(msg).toBe(describeError(e));
		expect(msg).toMatch(/video server/i);
		expect(msg).toMatch(/episode/i);
		expect(msg).not.toBe(m.play_play_failure_network());
		expect(msg).not.toBe(m.play_play_failure_generic());
		expect(msg).not.toBe(describeError({ kind: 'network' }));
		expect(msg).not.toBe(describeError({ kind: 'parse_failed' }));
		expect(msg).not.toContain('zokoanime.video');
		expect(msg).not.toContain('embed');
		// Every play surface, and the override point, read it the same.
		expect(describeExternalLaunchFailure(e, null)).toBe(msg);
		expect(describePlayFailure(e, { otherAudio: null, noResults: () => 'NOT IN CATALOGUE' })).toBe(
			msg
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
		expect(
			describePlayFailure(
				{ kind: 'no_results' },
				{ otherAudio: null, noResults: () => 'not indexed' }
			)
		).toBe('not indexed');
		expect(describePlayFailure({ kind: 'no_results' }, { otherAudio: null })).toBe(
			m.play_play_failure_no_results()
		);
		// The override touches nothing else.
		expect(
			describePlayFailure({ kind: 'timeout' }, { otherAudio: null, noResults: () => 'not indexed' })
		).toBe(m.play_play_failure_timeout());
	});
});

describe('wait copy reads right for any number of seconds', () => {
	// "{seconds} seconds" with one fixed noun is wrong for 1 in English
	// and for most numbers in Russian (1 секунду, 2 секунды, 5 секунд).
	// The wait is written with the unit's abbreviation, which does not
	// inflect, so it needs no plural variants.
	const UNIT = { en: 's', 'pt-BR': 's', 'es-419': 's', ru: 'с' } as const;
	const WAITS = [m.errors_reason_busy_wait, m.play_play_failure_rate_limited_wait];

	it('abbreviates the unit after the number in every locale', () => {
		for (const [locale, unit] of Object.entries(UNIT) as Array<[keyof typeof UNIT, string]>) {
			for (const wait of WAITS) {
				for (const seconds of [1, 2, 5, 21]) {
					const text = wait({ seconds }, { locale });
					expect(text, `${locale} ${seconds}`).toContain(`${seconds} ${unit}`);
					expect(text, `${locale} ${seconds}`).not.toMatch(
						new RegExp(`${seconds} ${unit}[A-Za-zА-Яа-яё]`)
					);
				}
			}
		}
	});
});

describe('describePlayFailure — an episode missing from this audio', () => {
	const e = { kind: 'episode_unavailable', key: 'error.play.episode_unavailable' };
	const OTHER_AUDIO = /other audio|outro áudio|otro audio|другую озвучку/i;

	it('suggests the other audio when the cache knows it has the episode', () => {
		const msg = describePlayFailure(e, { otherAudio: true });
		expect(msg).toBe(m.play_play_failure_episode_unavailable_try_other_audio());
		expect(msg).toMatch(OTHER_AUDIO);
	});

	it('does not send the user to an audio the cache knows has nothing', () => {
		// The defect: the sub cache held four episodes, the dub cache
		// held none, and the sentence still said to try the other
		// audio — which the detail page then hid Play for.
		const msg = describePlayFailure(e, { otherAudio: false });
		expect(msg).toBe(m.play_play_failure_episode_unavailable());
		expect(msg).not.toMatch(OTHER_AUDIO);
		expect(msg).toMatch(/another episode/i);
	});

	it('does not promise an audio nobody has asked about', () => {
		// Never probed, an answer the cache would not serve, or a
		// lookup that failed: nothing vouches for the other audio.
		const msg = describePlayFailure(e, { otherAudio: null });
		expect(msg).toBe(describePlayFailure(e, { otherAudio: false }));
		expect(msg).not.toMatch(OTHER_AUDIO);
	});

	it('passes the answer through the external-player and Syncplay copy', () => {
		expect(describeExternalLaunchFailure(e, true)).toBe(
			describePlayFailure(e, { otherAudio: true })
		);
		expect(describeExternalLaunchFailure(e, false)).toBe(
			describePlayFailure(e, { otherAudio: false })
		);
	});

	it('leaves every other branch alone whatever the other audio holds', () => {
		for (const other of [true, false, null]) {
			expect(describePlayFailure({ kind: 'timeout' }, { otherAudio: other })).toBe(
				m.play_play_failure_timeout()
			);
			expect(describePlayFailure({ kind: 'no_results' }, { otherAudio: other })).toBe(
				m.play_play_failure_no_results()
			);
		}
	});

	it('every locale drops the other-audio suggestion from the default sentence', () => {
		for (const locale of ['en', 'pt-BR', 'es-419', 'ru'] as const) {
			const plain = m.play_play_failure_episode_unavailable({}, { locale });
			const suggest = m.play_play_failure_episode_unavailable_try_other_audio({}, { locale });
			expect(plain, locale).not.toMatch(OTHER_AUDIO);
			expect(suggest, locale).toMatch(OTHER_AUDIO);
			if (locale !== 'en') {
				expect(plain, `${locale} is translated`).not.toBe(
					m.play_play_failure_episode_unavailable({}, { locale: 'en' })
				);
			}
		}
	});
});
