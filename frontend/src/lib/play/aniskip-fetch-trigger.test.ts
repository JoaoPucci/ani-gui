/**
 * Reactive-fetch trigger for aniskip — its whole job is to decide,
 * given the latest (showId, episodeNum, duration-ready) snapshot,
 * whether a fresh aniskip request should fire. The Player route's
 * `$effect` calls `step()` on each re-run and routes the resulting
 * decision to either a fetch, an interval clear, or a no-op.
 *
 * The reason this lives in its own helper: the original inline
 * `$effect` read `duration` reactively, so every `timeupdate` (~250
 * times a minute during playback) re-ran the effect body even though
 * the same-key guard short-circuited the fetch itself. Moving the
 * guard into a closure factory lets the component subscribe only to
 * the episode key and a `boolean` "is duration usable" derived —
 * both of which stay stable across duration ticks within an episode —
 * and lets us unit-test the same-key / clear / refetch transitions
 * without booting Svelte.
 */
import { describe, it, expect } from 'vitest';
import fc from 'fast-check';
import { createAniskipFetchTrigger, lengthReady } from './aniskip-fetch-trigger';

describe('aniskip fetch trigger', () => {
	it('fires a fetch on the first step where the key is complete and duration is ready', () => {
		const trigger = createAniskipFetchTrigger();
		expect(trigger.step('show-1', 5, true)).toEqual({
			kind: 'fetch',
			showId: 'show-1',
			episode: '5'
		});
	});

	it('does not refire on subsequent steps with the same key, regardless of how many times duration tick reruns the effect', () => {
		const trigger = createAniskipFetchTrigger();
		// First step issues the fetch.
		trigger.step('show-1', 5, true);
		// The Player's `timeupdate` would otherwise drive `duration` and
		// re-run the effect dozens of times per minute. Simulate that
		// burst and assert the trigger stays idle for every one.
		for (let i = 0; i < 1000; i++) {
			expect(trigger.step('show-1', 5, true)).toEqual({ kind: 'idle' });
		}
	});

	it('stays idle while duration is not yet ready', () => {
		const trigger = createAniskipFetchTrigger();
		expect(trigger.step('show-1', 5, false)).toEqual({ kind: 'idle' });
		expect(trigger.step('show-1', 5, false)).toEqual({ kind: 'idle' });
	});

	it('fires the fetch the moment duration flips to ready, even after prior not-ready steps', () => {
		const trigger = createAniskipFetchTrigger();
		trigger.step('show-1', 5, false);
		trigger.step('show-1', 5, false);
		expect(trigger.step('show-1', 5, true)).toEqual({
			kind: 'fetch',
			showId: 'show-1',
			episode: '5'
		});
	});

	it('refires when the episode changes within the same show', () => {
		const trigger = createAniskipFetchTrigger();
		trigger.step('show-1', 5, true);
		expect(trigger.step('show-1', 6, true)).toEqual({
			kind: 'fetch',
			showId: 'show-1',
			episode: '6'
		});
	});

	it('refires when the show changes', () => {
		const trigger = createAniskipFetchTrigger();
		trigger.step('show-1', 5, true);
		expect(trigger.step('show-2', 5, true)).toEqual({
			kind: 'fetch',
			showId: 'show-2',
			episode: '5'
		});
	});

	it('emits a clear when the key drops to incomplete after a fetch', () => {
		const trigger = createAniskipFetchTrigger();
		trigger.step('show-1', 5, true);
		expect(trigger.step('', 5, true)).toEqual({ kind: 'clear' });
	});

	it('stays idle when the key is incomplete and nothing was previously fetched', () => {
		const trigger = createAniskipFetchTrigger();
		expect(trigger.step('', 5, true)).toEqual({ kind: 'idle' });
		expect(trigger.step('show-1', 0, true)).toEqual({ kind: 'idle' });
	});

	it('clear is one-shot — repeated steps with an incomplete key after the clear stay idle', () => {
		const trigger = createAniskipFetchTrigger();
		trigger.step('show-1', 5, true);
		expect(trigger.step('', 5, true)).toEqual({ kind: 'clear' });
		expect(trigger.step('', 5, true)).toEqual({ kind: 'idle' });
	});

	it('treats episode 0 as incomplete (aniskip episodes are 1-indexed)', () => {
		const trigger = createAniskipFetchTrigger();
		expect(trigger.step('show-1', 0, true)).toEqual({ kind: 'idle' });
	});

	it('after a clear, the next complete-key+ready step re-issues a fetch', () => {
		const trigger = createAniskipFetchTrigger();
		trigger.step('show-1', 5, true);
		trigger.step('', 5, true); // clear
		expect(trigger.step('show-1', 5, true)).toEqual({
			kind: 'fetch',
			showId: 'show-1',
			episode: '5'
		});
	});
});

describe('createAniskipFetchTrigger — a fractional extra', () => {
	it('asks for no skip times on a recap', () => {
		// aniskip indexes episodes by MAL's whole-number list, where a
		// recap the provider numbers 7.5 has no entry: asking would at
		// best find nothing and at worst return another episode's.
		const t = createAniskipFetchTrigger();
		expect(t.step('42', 7.5, true)).toEqual({ kind: 'idle' });
	});

	it("clears the previous episode's skip times on moving to a recap", () => {
		const t = createAniskipFetchTrigger();
		expect(t.step('42', 7, true)).toEqual({ kind: 'fetch', showId: '42', episode: '7' });
		expect(t.step('42', 7.5, true)).toEqual({ kind: 'clear' });
		expect(t.step('42', 8, true)).toEqual({ kind: 'fetch', showId: '42', episode: '8' });
	});
});

describe('createAniskipFetchTrigger — an episode switch', () => {
	// Across a switch the page cannot ask for the new episode's times
	// until its stream has learned its own length; the last episode's
	// times are not this one's in the meantime.
	it("clears the last episode's times at once, before the new one's length is known", () => {
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		expect(t.step('42', 12, false)).toEqual({ kind: 'clear' });
		expect(t.step('42', 12, false)).toEqual({ kind: 'idle' });
		expect(t.step('42', 12, true)).toEqual({ kind: 'fetch', showId: '42', episode: '12' });
	});

	it('asks again for an episode switched back to', () => {
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		expect(t.step('42', 12, false)).toEqual({ kind: 'clear' });
		expect(t.step('42', 11, false)).toEqual({ kind: 'idle' });
		expect(t.step('42', 11, true)).toEqual({ kind: 'fetch', showId: '42', episode: '11' });
	});

	it('keeps the times of an episode opened again in a new session', () => {
		// A recovery or a reload: same episode, a new stream that has not
		// learned its length yet. Its times are still this episode's.
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		expect(t.step('42', 11, false)).toEqual({ kind: 'idle' });
		expect(t.step('42', 11, true)).toEqual({ kind: 'idle' });
	});

	it('stays idle on a first episode whose length is not yet known', () => {
		const t = createAniskipFetchTrigger();
		expect(t.step('42', 11, false)).toEqual({ kind: 'idle' });
	});
});

describe('createAniskipFetchTrigger — whose answer is shown', () => {
	it('shows the answer to the latest request', () => {
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		expect(t.answers(t.asked())).toBe(true);
	});

	it('drops an answer for the last episode once the episode changes, whatever its length', () => {
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		const ask = t.asked();
		t.step('42', 12, false);
		expect(t.answers(ask)).toBe(false);
	});

	it('drops an earlier request for the same episode once it is switched back to', () => {
		// 11 → 12 → 11: the first request for 11 answers late. The
		// times shown are the ones asked with the stream on screen.
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		const first = t.asked();
		t.step('42', 12, true);
		t.step('42', 11, true);
		expect(t.answers(first)).toBe(false);
		expect(t.answers(t.asked())).toBe(true);
	});

	it('drops an answer once the page moves to an extra', () => {
		const t = createAniskipFetchTrigger();
		t.step('42', 12, true);
		const ask = t.asked();
		t.step('42', 12.5, true);
		expect(t.answers(ask)).toBe(false);
	});

	it('keeps the answer for an episode opened again in a new session', () => {
		const t = createAniskipFetchTrigger();
		t.step('42', 11, true);
		const ask = t.asked();
		t.step('42', 11, false);
		t.step('42', 11, true);
		expect(t.answers(ask)).toBe(true);
	});

	it('only ever shows the answer to the latest request, over any run of steps', () => {
		const step = fc.record({
			episode: fc.constantFrom(0, 11, 12, 12.5),
			ready: fc.boolean()
		});
		fc.assert(
			fc.property(fc.array(step, { maxLength: 30 }), (steps) => {
				const t = createAniskipFetchTrigger();
				const asks: number[] = [];
				for (const s of steps) {
					if (t.step('42', s.episode, s.ready).kind === 'fetch') asks.push(t.asked());
				}
				const live = asks.filter((a) => t.answers(a));
				expect(live.length).toBeLessThanOrEqual(1);
				if (live.length === 1) expect(live[0]).toBe(asks.at(-1));
			})
		);
	});
});

describe('lengthReady', () => {
	it('is ready once the stream on screen has learned its length', () => {
		expect(lengthReady('session-2', 'session-2', 1500)).toBe(true);
	});

	it("is not ready while the length is still the last stream's", () => {
		// The page has moved to session-2; the element still describes
		// session-1's stream until session-2's attaches.
		expect(lengthReady('session-1', 'session-2', 1420)).toBe(false);
	});

	it('is not ready before any stream has attached', () => {
		expect(lengthReady(null, 'session-1', 1420)).toBe(false);
	});

	it('is not ready without a usable length', () => {
		for (const d of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
			expect(lengthReady('session-1', 'session-1', d)).toBe(false);
		}
	});

	it('is ready exactly when the sessions match and the length is usable', () => {
		fc.assert(
			fc.property(
				fc.option(fc.constantFrom('a', 'b')),
				fc.constantFrom('a', 'b'),
				fc.double(),
				(lengthFor, session, duration) => {
					expect(lengthReady(lengthFor, session, duration)).toBe(
						lengthFor === session && Number.isFinite(duration) && duration > 0
					);
				}
			)
		);
	});
});
