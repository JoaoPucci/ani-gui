import { describe, expect, it } from 'vitest';
import { BUFFER_GAP_TOLERANCE_S, bufferAheadSeconds, type BufferedRanges } from './buffer-ahead';

function ranges(...pairs: [number, number][]): BufferedRanges {
	return {
		length: pairs.length,
		start: (i) => pairs[i][0],
		end: (i) => pairs[i][1]
	};
}

describe('bufferAheadSeconds', () => {
	it('is zero with nothing buffered', () => {
		expect(bufferAheadSeconds(ranges(), 12)).toBe(0);
	});

	it('is zero when the playhead sits in no buffered range', () => {
		expect(bufferAheadSeconds(ranges([30, 60]), 12)).toBe(0);
	});

	it('is the run of media from the playhead to the end of its range', () => {
		expect(bufferAheadSeconds(ranges([0, 130]), 10)).toBe(120);
	});

	it('joins ranges a small gap apart, as the engine steps over one', () => {
		const gap = BUFFER_GAP_TOLERANCE_S / 2;
		expect(bufferAheadSeconds(ranges([0, 60], [60 + gap, 120]), 10)).toBeCloseTo(110, 5);
	});

	it('stops at a gap the engine would have to load across', () => {
		expect(bufferAheadSeconds(ranges([0, 60], [90, 120]), 10)).toBe(50);
	});

	it('reads the ranges in time order whichever order they come in', () => {
		expect(bufferAheadSeconds(ranges([90, 120], [0, 60]), 10)).toBe(50);
	});
});
