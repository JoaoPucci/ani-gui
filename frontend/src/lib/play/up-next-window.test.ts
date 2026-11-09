import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { END_SLACK_S, atVideoEnd, upNextShown, type UpNextState } from './up-next-window';

const LENGTH = 1420;

describe('atVideoEnd', () => {
	it('holds once the video has ended', () => {
		expect(atVideoEnd(LENGTH, LENGTH, true)).toBe(true);
		// The element's own word counts even where the length it reported is off.
		expect(atVideoEnd(LENGTH - 30, LENGTH, true)).toBe(true);
	});

	it('holds when the playhead stops just short of the length, as a stream can', () => {
		// hls.js can stall a fraction of a second before `duration`, so
		// `ended` never fires there.
		expect(atVideoEnd(LENGTH - 0.3, LENGTH, false)).toBe(true);
		expect(atVideoEnd(LENGTH - END_SLACK_S, LENGTH, false)).toBe(true);
		expect(atVideoEnd(LENGTH, LENGTH, false)).toBe(true);
	});

	it('does not hold in the credits, nor in the last minutes', () => {
		expect(atVideoEnd(1300, LENGTH, false)).toBe(false);
		expect(atVideoEnd(LENGTH - 60, LENGTH, false)).toBe(false);
		expect(atVideoEnd(LENGTH - END_SLACK_S - 0.1, LENGTH, false)).toBe(false);
	});

	it('decides nothing from an unknown length', () => {
		// The play page holds a length it has not learned as zero.
		expect(atVideoEnd(0, 0, false)).toBe(false);
		expect(atVideoEnd(0.5, 0, false)).toBe(false);
		expect(atVideoEnd(5000, Number.NaN, false)).toBe(false);
		expect(atVideoEnd(5000, Number.POSITIVE_INFINITY, false)).toBe(false);
	});

	it('is the ended state or the last moment of a known length, and nothing else', () => {
		fc.assert(
			fc.property(
				fc.oneof(
					fc.double({ min: 1, max: 5000, noNaN: true }),
					fc.constantFrom(0, Number.NaN, Number.POSITIVE_INFINITY)
				),
				fc.double({ min: 0, max: 5000, noNaN: true }),
				fc.boolean(),
				(duration, t, ended) => {
					const known = Number.isFinite(duration) && duration > 0;
					const want = ended || (known && duration - t <= END_SLACK_S);
					expect(atVideoEnd(t, duration, ended)).toBe(want);
				}
			)
		);
	});

	it('once reached on a known length, holds for every later point', () => {
		fc.assert(
			fc.property(
				fc.double({ min: 1, max: 5000, noNaN: true }),
				fc.double({ min: 0, max: 5000, noNaN: true }),
				fc.double({ min: 0, max: 200, noNaN: true }),
				(duration, t, later) => {
					if (atVideoEnd(t, duration, false))
						expect(atVideoEnd(t + later, duration, false)).toBe(true);
				}
			)
		);
	});
});

describe('upNextShown', () => {
	const due: UpNextState = {
		card: true,
		hasNext: false,
		resumeHolding: false,
		dismissed: false,
		videoFullscreen: false,
		atEnd: true
	};

	it('shows at the end of a finale', () => {
		expect(upNextShown(due)).toBe(true);
	});

	it('does not show before the end', () => {
		expect(upNextShown({ ...due, atEnd: false })).toBe(false);
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
					atEnd: fc.boolean()
				}),
				(state) => {
					const want =
						state.card &&
						!state.hasNext &&
						!state.resumeHolding &&
						!state.dismissed &&
						!state.videoFullscreen &&
						state.atEnd;
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
