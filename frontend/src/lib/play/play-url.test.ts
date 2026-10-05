import { describe, expect, it } from 'vitest';
import { buildPlayQuery } from './play-url';
import type { CreateSessionResponse } from '$lib/api';

const baseSession = (over: Partial<CreateSessionResponse> = {}): CreateSessionResponse => ({
	session_id: 's-123',
	media_url: 'http://localhost:9999/s/s-123/master.m3u8',
	media_kind: 'hls',
	cache_hit: false,
	...over
});

describe('buildPlayQuery', () => {
	it('includes session, episode, and kind', () => {
		const q = buildPlayQuery(baseSession({ session_id: 'abc', media_kind: 'mp4' }), 7);
		expect(q).toContain('session=abc');
		expect(q).toContain('episode=7');
		expect(q).toContain('kind=mp4');
	});

	it('url-encodes the session id (it can contain reserved chars)', () => {
		const q = buildPlayQuery(baseSession({ session_id: 'a/b c' }), 1);
		expect(q).toContain('session=a%2Fb%20c');
	});

	it('appends cache_hit=1 only when cache_hit is true', () => {
		expect(buildPlayQuery(baseSession({ cache_hit: true }), 1)).toContain('cache_hit=1');
		expect(buildPlayQuery(baseSession({ cache_hit: false }), 1)).not.toContain('cache_hit');
		// Older backend builds may omit the field entirely; treat absence
		// the same as false.
		const noField = baseSession();
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		delete (noField as any).cache_hit;
		expect(buildPlayQuery(noField, 1)).not.toContain('cache_hit');
	});

	it('combines all flags when cache_hit applies', () => {
		const q = buildPlayQuery(
			baseSession({
				session_id: 'sx',
				media_kind: 'hls',
				cache_hit: true
			}),
			12
		);
		// Order isn't part of the contract; URLSearchParams round-trip
		// proves every key landed regardless of join order.
		const params = new URLSearchParams(q.replace(/^\?/, ''));
		expect(params.get('session')).toBe('sx');
		expect(params.get('episode')).toBe('12');
		expect(params.get('kind')).toBe('hls');
		expect(params.get('cache_hit')).toBe('1');
	});

	it('starts with `?` so callers can append directly to the route base', () => {
		const q = buildPlayQuery(baseSession(), 1);
		expect(q.startsWith('?')).toBe(true);
	});

	it('carries no quality or mode: nothing reads them back', () => {
		// The play page read them to stamp the session a returning page
		// could take back; the page now loads every visit fresh, so the
		// URL names the session and nothing about how it was resolved.
		// @ts-expect-error the builder takes no quality or mode
		const q = buildPlayQuery(baseSession({ session_id: 's' }), 3, 'worst', 'dub');
		const p = new URLSearchParams(q.replace(/^\?/, ''));
		expect(p.get('q')).toBeNull();
		expect(p.get('md')).toBeNull();
	});

	it('omits q/md when not provided', () => {
		const q = buildPlayQuery(baseSession(), 1);
		expect(q).not.toContain('q=');
		expect(q).not.toContain('md=');
	});
});
