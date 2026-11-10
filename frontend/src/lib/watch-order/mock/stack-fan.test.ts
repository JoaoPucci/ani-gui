import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MOCK_RUNS } from './fixtures';
import { FAN_RADIUS_REM, fanPoses, stackCards, type FanPose } from './stack-fan';
import { franchiseSummary } from './strip-view';
import type { MockRun } from './types';

const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;
const near = (a: number, b: number) => Math.abs(a - b) < 1e-9;

describe('stackCards', () => {
	const posters = franchiseSummary(run('jojo')).posters;

	it('holds up to the limit of posters, in order', () => {
		expect(stackCards(posters, 3).map((c) => (c.kind === 'poster' ? c.entry.id : '-'))).toEqual(
			posters.map((p) => p.id)
		);
		expect(stackCards(posters, 2)).toHaveLength(2);
		expect(stackCards(posters, 1)).toEqual([{ kind: 'poster', entry: posters[0] }]);
	});

	it('holds three blank frames when no entry has a poster', () => {
		expect(stackCards([], 3)).toEqual([{ kind: 'blank' }, { kind: 'blank' }, { kind: 'blank' }]);
		expect(stackCards(posters, 0)).toEqual([
			{ kind: 'blank' },
			{ kind: 'blank' },
			{ kind: 'blank' }
		]);
	});
});

describe('fanPoses', () => {
	it("opens three cards to the second round's hovered fan: 9° apart, 1.3125rem to each side", () => {
		const open = fanPoses(3, 'open');
		expect(open.map((p) => p.rotate)).toEqual([-9, 0, 9]);
		expect(open[0].x).toBeCloseTo(-1.3125, 4);
		expect(open[2].x).toBeCloseTo(1.3125, 4);
		expect(open[1]).toEqual({ x: 0, y: 0, rotate: 0 });
	});

	it('rests three cards a little closer together, on the same arc', () => {
		const rest = fanPoses(3, 'rest');
		expect(rest.map((p) => p.rotate)).toEqual([-6.5, 0, 6.5]);
		expect(rest[2].x).toBeGreaterThan(0.8);
		expect(rest[2].x).toBeLessThan(fanPoses(3, 'open')[2].x);
	});

	it('splits two cards about the middle, half a step to each side', () => {
		expect(fanPoses(2, 'open').map((p) => p.rotate)).toEqual([-4.5, 4.5]);
		expect(fanPoses(2, 'rest').map((p) => p.rotate)).toEqual([-3.25, 3.25]);
	});

	it('stands one card upright in the middle, lifting it a little when open', () => {
		expect(fanPoses(1, 'rest')).toEqual([{ x: 0, y: 0, rotate: 0 }]);
		const [open] = fanPoses(1, 'open');
		expect(open.x).toBe(0);
		expect(open.rotate).toBe(0);
		expect(open.y).toBeLessThan(0);
	});

	it('has no pose for no card', () => {
		expect(fanPoses(0, 'open')).toEqual([]);
	});

	const count = fc.integer({ min: 0, max: 3 });
	const state = fc.constantFrom('rest' as const, 'open' as const);

	it('gives one pose per card, mirrored about the middle', () => {
		fc.assert(
			fc.property(count, state, (n, s) => {
				const poses = fanPoses(n, s);
				expect(poses).toHaveLength(n);
				poses.forEach((p, i) => {
					const twin = poses[n - 1 - i];
					expect(near(p.x, -twin.x)).toBe(true);
					expect(near(p.rotate, -twin.rotate)).toBe(true);
					expect(near(p.y, twin.y)).toBe(true);
				});
			})
		);
	});

	it('keeps the cards in reading order, leaning outwards', () => {
		fc.assert(
			fc.property(count, state, (n, s) => {
				const poses = fanPoses(n, s);
				for (let i = 1; i < n; i++) {
					expect(poses[i].x).toBeGreaterThan(poses[i - 1].x);
					expect(poses[i].rotate).toBeGreaterThan(poses[i - 1].rotate);
				}
			})
		);
	});

	it('moves every card outwards along one arc as it opens, never inwards', () => {
		const onArc = (p: FanPose) =>
			Math.abs(p.x ** 2 + (FAN_RADIUS_REM - p.y) ** 2 - FAN_RADIUS_REM ** 2) < 1e-6;
		fc.assert(
			fc.property(fc.integer({ min: 2, max: 3 }), (n) => {
				const rest = fanPoses(n, 'rest');
				const open = fanPoses(n, 'open');
				rest.forEach((r, i) => {
					const o = open[i];
					expect(onArc(r) && onArc(o)).toBe(true);
					expect(Math.abs(o.x)).toBeGreaterThanOrEqual(Math.abs(r.x));
					expect(Math.abs(o.rotate)).toBeGreaterThanOrEqual(Math.abs(r.rotate));
					if (r.x !== 0) expect(Math.abs(o.x)).toBeGreaterThan(Math.abs(r.x));
				});
			})
		);
	});
});
