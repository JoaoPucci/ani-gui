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
import { createAniskipFetchTrigger } from './aniskip-fetch-trigger';

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

	it('treats episode 0 as incomplete (ani-cli/aniskip episodes are 1-indexed)', () => {
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
