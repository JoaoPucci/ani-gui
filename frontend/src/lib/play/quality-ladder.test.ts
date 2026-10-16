import { describe, expect, it } from 'vitest';
import { ladderOrder, parseQualitySetting, targetLevel, type LadderLevel } from './quality-ladder';

// The master the defect was measured on: three levels, listed by
// hls.js lowest first.
const MASTER: LadderLevel[] = [
	{ height: 360, bitrate: 800_000 },
	{ height: 720, bitrate: 2_500_000 },
	{ height: 1080, bitrate: 5_000_000 }
];

describe('parseQualitySetting', () => {
	it('reads every value the settings page offers', () => {
		expect(parseQualitySetting('best')).toEqual({ kind: 'best' });
		expect(parseQualitySetting('worst')).toEqual({ kind: 'worst' });
		expect(parseQualitySetting('1080')).toEqual({ kind: 'height', height: 1080 });
		expect(parseQualitySetting('720')).toEqual({ kind: 'height', height: 720 });
		expect(parseQualitySetting('480')).toEqual({ kind: 'height', height: 480 });
	});

	it('reads anything else as best, the default the backend applies', () => {
		for (const s of [undefined, null, '', 'high', '0', '-720', '720p', '07']) {
			expect(parseQualitySetting(s)).toEqual({ kind: 'best' });
		}
	});
});

describe('ladderOrder', () => {
	it('orders by height, then by bitrate, whatever order the playlist used', () => {
		const levels: LadderLevel[] = [
			{ height: 1080, bitrate: 6_000_000 },
			{ height: 360, bitrate: 800_000 },
			{ height: 1080, bitrate: 4_000_000 },
			{ height: 0, bitrate: 9_000_000 }
		];
		expect(ladderOrder(levels)).toEqual([3, 1, 2, 0]);
	});

	it('is empty for no levels', () => {
		expect(ladderOrder([])).toEqual([]);
	});
});

describe('targetLevel', () => {
	it('best is the highest level and worst the lowest', () => {
		expect(targetLevel({ kind: 'best' }, MASTER)).toBe(2);
		expect(targetLevel({ kind: 'worst' }, MASTER)).toBe(0);
	});

	it('a height takes its own level when the master has it', () => {
		expect(targetLevel({ kind: 'height', height: 720 }, MASTER)).toBe(1);
		expect(targetLevel({ kind: 'height', height: 1080 }, MASTER)).toBe(2);
	});

	it('a height the master lacks takes the highest level below it', () => {
		// The backend matched 480 against RESOLUTION tags, missed, and
		// handed over the master: the 360 level is what fits.
		expect(targetLevel({ kind: 'height', height: 480 }, MASTER)).toBe(0);
		expect(targetLevel({ kind: 'height', height: 900 }, MASTER)).toBe(1);
	});

	it('a height below every level takes the lowest', () => {
		expect(targetLevel({ kind: 'height', height: 240 }, MASTER)).toBe(0);
	});

	it('takes the higher bitrate of two levels of one height', () => {
		const twins: LadderLevel[] = [
			{ height: 1080, bitrate: 6_000_000 },
			{ height: 1080, bitrate: 4_000_000 }
		];
		expect(targetLevel({ kind: 'best' }, twins)).toBe(0);
		expect(targetLevel({ kind: 'height', height: 1080 }, twins)).toBe(0);
	});

	it('is -1 when there are no levels', () => {
		expect(targetLevel({ kind: 'best' }, [])).toBe(-1);
		expect(targetLevel({ kind: 'worst' }, [])).toBe(-1);
		expect(targetLevel({ kind: 'height', height: 720 }, [])).toBe(-1);
	});
});
