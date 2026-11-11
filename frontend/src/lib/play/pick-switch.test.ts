// "Switch to it now": the lifecycle around the switch on the player.

import { describe, expect, it, vi } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';
import type { AudioMode } from './audio-choice';
import type { SwitchOutcome, SwitchRequest } from './switch-show';
import { switchToPick, type Pick, type PickSwitchDeps } from './pick-switch';

const PICK: Pick = {
	id: '4013',
	title: 'JoJo no Kimyou na Bouken: Adventure',
	show: { id: '4013', canonical_title: 'JoJo no Kimyou na Bouken: Adventure' } as KitsuAnimeRef
};

interface World {
	deps: PickSwitchDeps;
	busy: boolean;
	busyLog: boolean[];
	carried: { showId: string; audio: AudioMode } | null;
	requests: SwitchRequest[];
	log: string[];
}

function world(
	over: Partial<PickSwitchDeps> = {},
	outcome: SwitchOutcome | (() => Promise<SwitchOutcome>) = 'switched',
	playing = true
): World {
	const w: World = {
		busy: false,
		busyLog: [],
		carried: null,
		requests: [],
		log: [],
		deps: undefined as unknown as PickSwitchDeps
	};
	w.deps = {
		busy: () => w.busy,
		setBusy: (b) => {
			w.busy = b;
			w.busyLog.push(b);
		},
		playback: () => ({ at: 74, playing, rate: 1.5 }),
		pause: () => w.log.push('pause'),
		resume: () => w.log.push('resume'),
		storedAudio: () => undefined,
		sessionAudio: () => 'dub',
		carryAudio: (showId, audio) => {
			w.carried = { showId, audio };
		},
		takeAudio: () => {
			w.carried = null;
		},
		choices: () => ({ version: 'uncensored', caption: 'en' }),
		episode: () => 2,
		row: () => 'hianime:jojos-bizarre-adventure-2000-5182',
		run: async (req) => {
			w.requests.push(req);
			// The audio is carried while the switch runs.
			w.log.push(`run:${w.carried?.audio ?? 'none'}:${w.busy}`);
			return typeof outcome === 'function' ? outcome() : outcome;
		},
		failed: (p) => w.log.push(`failed:${p.id}`),
		...over
	};
	return w;
}

describe('switchToPick', () => {
	it('does nothing with no pick, or while the lock is held', async () => {
		const w = world();
		expect(await switchToPick(null, w.deps)).toBe('skipped');
		w.busy = true;
		expect(await switchToPick(PICK, w.deps)).toBe('skipped');
		expect(w.log).toEqual([]);
		expect(w.requests).toEqual([]);
	});

	it('pauses, carries the audio, holds the lock while it runs, and asks for the same episode and point', async () => {
		const w = world();
		expect(await switchToPick(PICK, w.deps)).toBe('switched');
		expect(w.log).toEqual(['pause', 'run:dub:true']);
		expect(w.requests).toEqual([
			{
				to: PICK.show,
				episode: 2,
				at: 74,
				row: 'hianime:jojos-bizarre-adventure-2000-5182',
				mode: 'dub',
				version: 'uncensored',
				caption: 'en',
				rate: 1.5
			}
		]);
		expect(w.busyLog).toEqual([true, false]);
		// The pick's player took the audio as it mounted: none is left.
		expect(w.carried).toBeNull();
	});

	it("plays the pick in its own stored audio, else the session's", async () => {
		const w = world({ storedAudio: (id) => (id === PICK.id ? 'sub' : undefined) });
		await switchToPick(PICK, w.deps);
		expect(w.requests[0].mode).toBe('sub');
		expect(w.log).toContain('run:sub:true');
	});

	it('says a failed switch failed, plays the episode on again if it was playing, and leaves nothing carried', async () => {
		const w = world({}, 'failed', true);
		expect(await switchToPick(PICK, w.deps)).toBe('failed');
		expect(w.log).toEqual(['pause', 'run:dub:true', `failed:${PICK.id}`, 'resume']);
		expect(w.carried).toBeNull();
		expect(w.busy).toBe(false);
	});

	it('leaves a paused episode paused when its switch fails', async () => {
		const w = world({}, 'failed', false);
		await switchToPick(PICK, w.deps);
		expect(w.log).not.toContain('resume');
		expect(w.log).toContain(`failed:${PICK.id}`);
	});

	it('says nothing and resumes nothing once the viewer left', async () => {
		const w = world({}, 'abandoned', true);
		expect(await switchToPick(PICK, w.deps)).toBe('abandoned');
		expect(w.log).toEqual(['pause', 'run:dub:true']);
		expect(w.carried).toBeNull();
		expect(w.busy).toBe(false);
	});

	it('treats a switch that threw as failed: says so, plays on, leaves nothing carried and frees the lock', async () => {
		// One of the switch's own effects threw (a storage write refused
		// as the point is carried over, say).
		const w = world({}, async () => {
			throw new Error('storage refused');
		});
		expect(await switchToPick(PICK, w.deps)).toBe('failed');
		expect(w.log).toEqual(['pause', 'run:dub:true', `failed:${PICK.id}`, 'resume']);
		expect(w.carried).toBeNull();
		expect(w.busy).toBe(false);
		// And the next click switches again.
		w.deps.run = async () => 'switched';
		expect(await switchToPick(PICK, w.deps)).toBe('switched');
	});

	it('takes one switch at a time', async () => {
		let release!: (o: SwitchOutcome) => void;
		const w = world({}, () => new Promise<SwitchOutcome>((r) => (release = r)));
		const first = switchToPick(PICK, w.deps);
		await vi.waitFor(() => expect(w.requests.length).toBe(1));
		expect(await switchToPick(PICK, w.deps)).toBe('skipped');
		release('switched');
		expect(await first).toBe('switched');
		expect(w.requests.length).toBe(1);
	});
});
