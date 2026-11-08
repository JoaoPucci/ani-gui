import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { SkipInterval } from '$lib/api';
import { FINISHED_REMAINING_S } from './watch-position';
import { creditsStart, inCredits, upNextShown, type UpNextState } from './up-next-window';

const ed = (start: number, end: number, skip_type = 'ed'): SkipInterval => ({
	skip_type,
	start_time: start,
	end_time: end
});
const OP = ed(60, 150, 'op');

describe('creditsStart', () => {
	it('is where the ending interval starts', () => {
		expect(creditsStart([OP, ed(1300, 1390)])).toBe(1300);
		expect(creditsStart([ed(1310, 1400, 'mixed-ed')])).toBe(1310);
	});

	it('takes the latest ending when there are several', () => {
		expect(creditsStart([ed(1300, 1390), ed(20, 60)])).toBe(1300);
	});

	it('is unknown without an ending', () => {
		expect(creditsStart([])).toBeNull();
		expect(creditsStart([OP, ed(0, 40, 'recap'), ed(10, 50, 'mixed-op')])).toBeNull();
	});
});

describe('inCredits', () => {
	it('holds from the ending on, past it, and at the end', () => {
		const skips = [OP, ed(1300, 1390)];
		expect(inCredits(1299.9, 1420, skips)).toBe(false);
		expect(inCredits(1300, 1420, skips)).toBe(true);
		// Auto-skip lands past the ending: the card is still due.
		expect(inCredits(1390.05, 1420, skips)).toBe(true);
		expect(inCredits(1420, 1420, skips)).toBe(true);
	});

	it("without an ending, holds in the episode's last minutes", () => {
		const from = 1420 - FINISHED_REMAINING_S;
		expect(inCredits(from - 0.1, 1420, [OP])).toBe(false);
		expect(inCredits(from, 1420, [OP])).toBe(true);
		expect(inCredits(1420, 1420, [])).toBe(true);
	});

	it('decides nothing while the length is unknown and no ending is known', () => {
		expect(inCredits(5000, Number.NaN, [])).toBe(false);
		// The player holds a length it has not learned yet as zero.
		expect(inCredits(0, 0, [])).toBe(false);
		expect(inCredits(10, 0, [OP])).toBe(false);
		expect(inCredits(1300, Number.NaN, [ed(1300, 1390)])).toBe(true);
	});

	it('once reached, holds for every later point', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.record({
						skip_type: fc.constantFrom('op', 'ed', 'mixed-ed', 'mixed-op', 'recap'),
						start_time: fc.double({ min: 0, max: 1500, noNaN: true }),
						end_time: fc.double({ min: 0, max: 1600, noNaN: true })
					}),
					{ maxLength: 4 }
				),
				fc.oneof(fc.double({ min: 1, max: 1600, noNaN: true }), fc.constant(Number.NaN)),
				fc.double({ min: 0, max: 1600, noNaN: true }),
				fc.double({ min: 0, max: 200, noNaN: true }),
				(skips, duration, t, later) => {
					if (inCredits(t, duration, skips))
						expect(inCredits(t + later, duration, skips)).toBe(true);
					const start = creditsStart(skips);
					if (start !== null) expect(inCredits(t, duration, skips)).toBe(t >= start);
				}
			)
		);
	});
});

describe('inCredits before the skip times answer', () => {
	it('waits for them rather than deciding by the last minutes', () => {
		expect(inCredits(1400, 1420, null)).toBe(false);
		expect(inCredits(1420, 1420, null)).toBe(false);
	});
});

describe('upNextShown', () => {
	const due: UpNextState = {
		card: true,
		hasNext: false,
		resumeHolding: false,
		dismissed: false,
		videoFullscreen: false,
		inCredits: true
	};

	it('shows at a finale in its credits', () => {
		expect(upNextShown(due)).toBe(true);
	});

	it('shows only when every condition holds', () => {
		fc.assert(
			fc.property(
				fc.record({
					card: fc.boolean(),
					hasNext: fc.boolean(),
					resumeHolding: fc.boolean(),
					dismissed: fc.boolean(),
					videoFullscreen: fc.boolean(),
					inCredits: fc.boolean()
				}),
				(state) => {
					const want =
						state.card &&
						!state.hasNext &&
						!state.resumeHolding &&
						!state.dismissed &&
						!state.videoFullscreen &&
						state.inCredits;
					expect(upNextShown(state)).toBe(want);
				}
			)
		);
	});

	it('waits while the player still has an episode of the entry to step to', () => {
		expect(upNextShown({ ...due, hasNext: true })).toBe(false);
	});

	it('is not drawn where only the video is fullscreen', () => {
		expect(upNextShown({ ...due, videoFullscreen: true })).toBe(false);
	});
});
