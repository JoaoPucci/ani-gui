import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { resumeStartAt } from './resume-hold';

// A point is a position in seconds; null is no point at all.
const point = fc.option(fc.double({ min: -10, max: 10_000, noNaN: true }), { nil: null });

describe('resumeStartAt', () => {
	it("a pending recovery's point wins over the kept one", () => {
		expect(resumeStartAt(42, 600)).toBe(42);
	});

	it('the kept point when no recovery is pending', () => {
		expect(resumeStartAt(null, 600)).toBe(600);
	});

	it('nowhere to start from: no point, or a started mark at zero', () => {
		expect(resumeStartAt(null, null)).toBeNull();
		expect(resumeStartAt(null, 0)).toBeNull();
		expect(resumeStartAt(0, 600)).toBeNull();
	});

	it('is the point that decides, and only a point past zero is one', () => {
		fc.assert(
			fc.property(point, point, (recovered, kept) => {
				const decided = recovered ?? kept;
				const at = resumeStartAt(recovered, kept);
				if (decided !== null && decided > 0) expect(at).toBe(decided);
				else expect(at).toBeNull();
			})
		);
	});
});
