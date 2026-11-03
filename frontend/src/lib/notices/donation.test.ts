import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { GRACE_MS, decide, parseState, serializeState, type DonationState } from './donation';

const DAY = 24 * 3600_000;
const T0 = 1_800_000_000_000;

/** A launch at `now` seeing `n`, from `state`. */
const launch = (state: DonationState | null, n: number | null, now: number, playing = false) =>
	decide(state, { n, now, playing });

describe('the donation ask', () => {
	it('records a new install and the number current at install, and asks nothing', () => {
		const r = launch(null, 3, T0);
		expect(r.ask).toBe(false);
		expect(r.state).toEqual({ v: 1, firstLaunchAt: T0, baseline: 3, acted: null, optOut: false });
	});

	it('asks once when the number rises past the last one acted on, after the grace period', () => {
		let s = launch(null, 3, T0).state;
		expect(launch(s, 3, T0 + 20 * DAY).ask).toBe(false);
		let r = launch(s, 4, T0 + 20 * DAY);
		expect(r.ask).toBe(true);
		s = r.state;
		expect(s.acted).toBe(4);
		expect(launch(s, 4, T0 + 21 * DAY).ask).toBe(false);
		r = launch(s, 5, T0 + 22 * DAY);
		expect(r.ask).toBe(true);
	});

	it('waits out the first fourteen days, and asks after them if the number is still unseen', () => {
		const s = launch(null, 3, T0).state;
		expect(launch(s, 4, T0 + GRACE_MS - 1).ask).toBe(false);
		expect(launch(s, 4, T0 + GRACE_MS).ask).toBe(true);
		expect(GRACE_MS).toBe(14 * DAY);
	});

	it('never asks while a video plays; it waits for a moment with nothing playing', () => {
		const s = launch(null, 1, T0).state;
		const playing = launch(s, 2, T0 + 30 * DAY, true);
		expect(playing.ask).toBe(false);
		expect(playing.state).toEqual(s);
		expect(launch(playing.state, 2, T0 + 30 * DAY).ask).toBe(true);
	});

	it('takes the first number it sees as the baseline when the install saw none', () => {
		let s = launch(null, null, T0).state;
		expect(s.baseline).toBeNull();
		const r = launch(s, 7, T0 + 30 * DAY);
		expect(r.ask).toBe(false);
		s = r.state;
		expect(s.baseline).toBe(7);
		expect(launch(s, 8, T0 + 31 * DAY).ask).toBe(true);
	});

	it('asks nothing when the file says nothing', () => {
		const s = launch(null, 2, T0).state;
		const r = launch(s, null, T0 + 30 * DAY);
		expect(r.ask).toBe(false);
		expect(r.state).toEqual(s);
	});

	it('honours the reserved opt-out, which nothing sets yet', () => {
		const s = { ...launch(null, 1, T0).state, optOut: true };
		expect(launch(s, 9, T0 + 90 * DAY).ask).toBe(false);
	});

	it('does not count a clock set back as time passed', () => {
		const s = launch(null, 1, T0).state;
		expect(launch(s, 2, T0 - 30 * DAY).ask).toBe(false);
	});
});

describe('stored state', () => {
	it('round-trips, and reads junk or another version as nothing stored', () => {
		const s = launch(null, 3, T0).state;
		expect(parseState(serializeState(s))).toEqual(s);
		for (const raw of [null, '', '{', 'null', '[]', '{"v":2}', '{"v":1,"firstLaunchAt":"x"}']) {
			expect(parseState(raw)).toBeNull();
		}
		expect(
			parseState('{"v":1,"firstLaunchAt":1,"baseline":-1,"acted":1.5,"optOut":"yes"}')
		).toEqual({ v: 1, firstLaunchAt: 1, baseline: null, acted: null, optOut: false });
	});
});

describe('over any sequence of launches', () => {
	const step = fc.record({
		n: fc.option(fc.integer({ min: 0, max: 6 }), { nil: null }),
		dt: fc.integer({ min: -2 * DAY, max: 10 * DAY }),
		playing: fc.boolean(),
		reinstall: fc.boolean()
	});

	it('never asks during playback, within the grace period, twice for a number, or for one at install', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 0, max: 6 }),
				fc.array(step, { maxLength: 40 }),
				(n0, steps) => {
					let state: DonationState | null = null;
					let now = T0;
					let installedAt = T0;
					let installN: number | null = n0;
					state = launch(state, n0, now).state;
					const asked = new Set<number>();
					for (const s of steps) {
						now += s.dt;
						if (s.reinstall) {
							state = null;
							installedAt = now;
							installN = s.n;
						}
						const r = launch(state, s.n, now, s.playing);
						if (r.ask) {
							expect(s.playing).toBe(false);
							expect(now - installedAt).toBeGreaterThanOrEqual(GRACE_MS);
							expect(s.n).not.toBeNull();
							expect(asked.has(s.n as number) && !s.reinstall).toBe(false);
							expect(installN === null || (s.n as number) > installN).toBe(true);
							asked.add(s.n as number);
						}
						if (s.reinstall) asked.clear();
						state = r.state;
					}
				}
			)
		);
	});
});
