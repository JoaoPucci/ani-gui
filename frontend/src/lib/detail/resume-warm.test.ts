import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CreateSessionResponse, PlayArgs } from '$lib/api';
import { __resetPlayCacheForTests, clearForShow, getOrFire, makeKey } from '$lib/play/play-cache';
import { warmResumeEpisode, type ResumeWarmStream } from './resume-warm';

const key = { showId: '42', episode: 12.5, mode: 'sub', quality: 'best' };
const args: PlayArgs = { title: 'Show', episode: '12.5', mode: 'sub', quality: 'best' };
const session: CreateSessionResponse = {
	session_id: 's',
	media_url: 'http://x/s/s/master.m3u8',
	media_kind: 'hls'
};

/** A stream that answers only when told to, recording each call. */
function heldStream() {
	const calls: Array<{ args: PlayArgs; signal?: AbortSignal; answer: () => void }> = [];
	const stream: ResumeWarmStream = (a, _p, signal) =>
		new Promise((resolve, reject) => {
			signal?.addEventListener('abort', () => reject(new Error('aborted')));
			calls.push({ args: a, signal, answer: () => resolve(session) });
		});
	return { stream, calls };
}

/** Fills the play cache's background slots with prefetches that never
 *  answer. */
function fillPrefetchSlots() {
	for (const ep of [1, 2]) {
		getOrFire(makeKey('other', ep, 'sub', 'best'), () => new Promise(() => {}));
	}
}

const settle = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => {
	__resetPlayCacheForTests();
});

describe('warmResumeEpisode', () => {
	it('resolves the episode as a prefetch, which records no watch', async () => {
		const { stream, calls } = heldStream();
		void warmResumeEpisode(key, args, stream);
		await settle();
		expect(calls).toHaveLength(1);
		expect(calls[0].args).toEqual({ ...args, prefetch: true });
	});

	// A warm left running after its page holds a connection on the
	// resolve lane until the resolve ends; enough of them, from page
	// after page, and the user's own play waits for a socket.
	it('is cancelled when the page that asked for it is left', async () => {
		const { stream, calls } = heldStream();
		const warm = warmResumeEpisode(key, args, stream);
		await settle();
		clearForShow(key.showId);
		expect(calls[0].signal?.aborted).toBe(true);
		await expect(warm).rejects.toThrow();
	});

	it('is the resolve a click on the same episode uses, not a second one', async () => {
		const { stream, calls } = heldStream();
		const warm = warmResumeEpisode(key, args, stream);
		await settle();
		calls[0].answer();
		await warm;
		const click = vi.fn(() => Promise.resolve(session));
		const played = await getOrFire(
			makeKey(key.showId, key.episode, key.mode, key.quality),
			click,
			() => {}
		);
		expect(played).toBe(session);
		expect(click).not.toHaveBeenCalled();
	});

	// Each version of a show is its own listing: the warm of the
	// version the row would play is the one a click on it reads, and a
	// click on the other version must not be served it.
	it('is keyed by the version it resolves', async () => {
		const { stream, calls } = heldStream();
		const standard = { ...key, version: 'standard' as const };
		const warm = warmResumeEpisode(standard, { ...args, version: 'standard' }, stream);
		await settle();
		calls[0].answer();
		await warm;
		const same = vi.fn(() => Promise.resolve(session));
		await getOrFire(
			makeKey(key.showId, key.episode, key.mode, key.quality, 'standard'),
			same,
			() => {}
		);
		expect(same).not.toHaveBeenCalled();
		const other = vi.fn(() => Promise.resolve(session));
		await getOrFire(makeKey(key.showId, key.episode, key.mode, key.quality), other, () => {});
		expect(other).toHaveBeenCalledTimes(1);
	});

	// The backend paces a prefetch as background, and its gate can
	// refuse one; a click must never be left waiting on it.
	it('leaves a click that comes while it runs its own resolve', async () => {
		const { stream, calls } = heldStream();
		const warm = warmResumeEpisode(key, args, stream);
		void warm.catch(() => {});
		await settle();
		expect(calls).toHaveLength(1);
		const click = vi.fn(() => Promise.resolve(session));
		const played = await getOrFire(
			makeKey(key.showId, key.episode, key.mode, key.quality),
			click,
			() => {}
		);
		expect(played).toBe(session);
		expect(click).toHaveBeenCalledTimes(1);
	});

	// A click on Play waits for the resume row to be read again, and
	// the read waits for this warm: queued behind the next episodes'
	// prefetches, the click would wait for them too.
	it('does not wait behind the background prefetches', async () => {
		fillPrefetchSlots();
		const { stream, calls } = heldStream();
		void warmResumeEpisode(key, args, stream);
		await settle();
		expect(calls).toHaveLength(1);
	});
});
