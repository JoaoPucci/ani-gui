import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { learnEncoded, learnPicture } from './quality-learn';
import { QualityTrack } from './quality-track';

function track(levels: number) {
	const t = new QualityTrack();
	t.arrive(Array.from({ length: levels }, () => ({ height: 0, bitrate: 0 })));
	return t;
}

describe('learnPicture', () => {
	it('names a lone level by a positive height it did not already have, and nothing else', () => {
		fc.assert(
			fc.property(
				fc.nat({ max: 4 }),
				fc.integer({ min: -1, max: 2160 }),
				fc.option(fc.integer({ min: 1, max: 2160 }), { nil: null }),
				(levels, height, known) => {
					const t = track(levels);
					if (known !== null && levels === 1) learnPicture(t, known);
					const news = levels === 1 && height > 0 && height !== known;
					expect(learnPicture(t, height)).toBe(news);
					if (levels === 1 && height > 0) expect(t.levels()[0].height).toBe(height);
					if (levels > 1) expect(t.levels().every((l) => l.height === 0)).toBe(true);
				}
			)
		);
	});
});

describe('learnEncoded', () => {
	it('logs a level encoded at another height than its playlist lists', () => {
		const t = new QualityTrack();
		t.arrive([{ height: 360, bitrate: 1 }]);
		const logs: string[] = [];
		const init = (height: number) => ({
			frag: { type: 'main', level: 0 },
			tracks: { video: { metadata: { height } } }
		});
		expect(learnEncoded(t, init(480), (l) => logs.push(l))).toBe(true);
		expect(logs).toEqual(['level 0 encodes 480p; its playlist says 360p']);
		expect(learnEncoded(t, init(480), (l) => logs.push(l))).toBe(false);
		expect(learnEncoded(t, { frag: { type: 'main', level: 0 }, tracks: {} }, () => {})).toBe(false);
	});
});
