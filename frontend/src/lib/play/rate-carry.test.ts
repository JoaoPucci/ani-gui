// The playback speed a switch to another show's player carries over.

import { beforeEach, describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { applyCarriedRate, carryRate, rateFor, takeRate, __resetRateForTests } from './rate-carry';

beforeEach(() => __resetRateForTests());

describe('the carried playback speed', () => {
	it('is handed once to the next player of the show it was carried to', () => {
		carryRate('4013', 1.5);
		expect(takeRate('4013')).toBe(1.5);
		expect(takeRate('4013')).toBeNull();
	});

	it('is seen without being taken, so a stream attached again before it plays still has it', () => {
		carryRate('4013', 1.5);
		expect(rateFor('4013')).toBe(true);
		expect(rateFor('7158')).toBe(false);
		expect(takeRate('4013')).toBe(1.5);
		expect(rateFor('4013')).toBe(false);
	});

	it('is dropped by a player of another show', () => {
		carryRate('4013', 1.5);
		expect(takeRate('7158')).toBeNull();
		expect(takeRate('4013')).toBeNull();
	});

	it('carries no normal speed, and no speed that is not one', () => {
		fc.assert(
			fc.property(fc.oneof(fc.constant(1), fc.double({ noNaN: false })), (rate) => {
				__resetRateForTests();
				carryRate('4013', rate);
				const got = takeRate('4013');
				if (rate === 1 || !Number.isFinite(rate) || rate <= 0) expect(got).toBeNull();
				else expect(got).toBe(rate);
			})
		);
	});
});

describe('applyCarriedRate', () => {
	function video() {
		const listeners = new Map<string, { fn: () => void; once: boolean }>();
		return {
			playbackRate: 1,
			addEventListener: (name: string, fn: () => void, opts?: AddEventListenerOptions) =>
				void listeners.set(name, { fn, once: opts?.once === true }),
			removeEventListener: (name: string, fn: () => void) => {
				if (listeners.get(name)?.fn === fn) listeners.delete(name);
			},
			fire: (name: string) => listeners.get(name)?.fn(),
			once: (name: string) => listeners.get(name)?.once === true,
			has: (name: string) => listeners.has(name)
		};
	}

	it('plays the carried speed once the metadata loads, and takes it then', () => {
		carryRate('4013', 1.5);
		const v = video();
		const stop = applyCarriedRate(v as unknown as HTMLMediaElement, '4013');
		expect(stop).not.toBeNull();
		// Once: a later load of the same element is not this switch's.
		expect(v.once('loadedmetadata')).toBe(true);
		// Not before: the load resets the rate.
		expect(v.playbackRate).toBe(1);
		expect(rateFor('4013')).toBe(true);
		v.fire('loadedmetadata');
		expect(v.playbackRate).toBe(1.5);
		expect(rateFor('4013')).toBe(false);
	});

	it('leaves the speed for a stream attached again when the source went first', () => {
		carryRate('4013', 1.5);
		const v = video();
		applyCarriedRate(v as unknown as HTMLMediaElement, '4013')!();
		expect(v.has('loadedmetadata')).toBe(false);
		expect(rateFor('4013')).toBe(true);
	});

	it('waits for nothing when no speed is carried to the show', () => {
		carryRate('9999', 1.5);
		const v = video();
		expect(applyCarriedRate(v as unknown as HTMLMediaElement, '4013')).toBeNull();
		expect(v.has('loadedmetadata')).toBe(false);
		// The other show's speed is left for its own player.
		expect(rateFor('9999')).toBe(true);
	});

	it('leaves the rate alone when the speed was taken before the metadata loaded', () => {
		carryRate('4013', 1.5);
		const v = video();
		applyCarriedRate(v as unknown as HTMLMediaElement, '4013');
		takeRate('4013');
		v.fire('loadedmetadata');
		expect(v.playbackRate).toBe(1);
	});
});
