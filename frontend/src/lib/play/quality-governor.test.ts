import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	HEALTHY_BUFFER_S,
	HEALTHY_WINDOWS_MS,
	QualityGovernor,
	SEEK_GRACE_MS,
	STEP_DOWN_COOLDOWN_MS
} from './quality-governor';
import type { LadderLevel, QualityPinChoice, QualityTarget } from './quality-ladder';

const MASTER: LadderLevel[] = [
	{ height: 360, bitrate: 800_000 },
	{ height: 720, bitrate: 2_500_000 },
	{ height: 1080, bitrate: 5_000_000 }
];
const BEST: QualityTarget = { kind: 'best' };
const FULL = HEALTHY_BUFFER_S + 5;

function governor(
	target: QualityTarget = BEST,
	pin: QualityPinChoice | null = null,
	levels = MASTER
) {
	return new QualityGovernor(levels, target, pin);
}

describe('QualityGovernor — the start', () => {
	it('best starts at the highest level, without flushing anything', () => {
		const g = governor();
		expect(g.start()).toEqual({ level: 2, from: null, reason: 'start', via: 'load' });
		expect(g.level).toBe(2);
		expect(g.isEngaged).toBe(true);
		expect(g.isPinned).toBe(false);
	});

	it('worst starts at the lowest and a height at its own level', () => {
		expect(governor({ kind: 'worst' }).start()?.level).toBe(0);
		expect(governor({ kind: 'height', height: 720 }).start()?.level).toBe(1);
	});

	it('a pinned height starts pinned at that height, or the nearest below it', () => {
		const exact = governor(BEST, 720);
		expect(exact.start()?.level).toBe(1);
		expect(exact.isPinned).toBe(true);
		expect(governor(BEST, 480).start()?.level).toBe(0);
	});

	it('a pin by bandwidth rank starts at that rank from the top, or the lowest past the end', () => {
		const top = governor(BEST, { rank: 0 });
		expect(top.start()?.level).toBe(2);
		expect(top.isPinned).toBe(true);
		expect(governor(BEST, { rank: 1 }).start()?.level).toBe(1);
		expect(governor(BEST, { rank: 9 }).start()?.level).toBe(0);
	});

	it('leaves a stream of one level, or none, to the engine', () => {
		for (const levels of [[MASTER[1]], []]) {
			const g = governor(BEST, null, levels);
			expect(g.isEngaged).toBe(false);
			expect(g.start()).toBeNull();
			expect(g.stall(100_000)).toBeNull();
			expect(g.pick(0, 100_000)).toBeNull();
		}
	});
});

describe('QualityGovernor — stalls step down', () => {
	it('one level per stall, at most once per cooldown, never below the lowest', () => {
		const g = governor();
		g.start();
		expect(g.stall(10_000)).toEqual({ level: 1, from: 2, reason: 'stall', via: 'load' });
		expect(g.stall(10_000 + STEP_DOWN_COOLDOWN_MS - 1)).toBeNull();
		expect(g.level).toBe(1);
		expect(g.stall(10_000 + STEP_DOWN_COOLDOWN_MS)?.level).toBe(0);
		expect(g.stall(100_000)).toBeNull();
		expect(g.level).toBe(0);
	});

	it('a stall inside the grace after a seek is the seek waiting for media', () => {
		const g = governor();
		g.start();
		g.seeking(50_000);
		expect(g.stall(50_000 + SEEK_GRACE_MS - 1)).toBeNull();
		expect(g.stall(50_000 + SEEK_GRACE_MS)?.level).toBe(1);
	});

	it('a pinned level does not move for a stall', () => {
		const g = governor(BEST, 1080);
		g.start();
		expect(g.stall(10_000)).toBeNull();
		expect(g.level).toBe(2);
	});
});

describe('QualityGovernor — healthy playback steps back up', () => {
	it('never moves a level that has not stepped down', () => {
		const g = governor();
		g.start();
		expect(g.healthy(1_000_000, FULL)).toBeNull();
	});

	it('steps up one level once the window has passed with a buffer in hand', () => {
		const g = governor();
		g.start();
		g.stall(10_000);
		const after = 10_000 + HEALTHY_WINDOWS_MS[0];
		expect(g.healthy(after - 1, FULL)).toBeNull();
		expect(g.healthy(after, HEALTHY_BUFFER_S - 0.1)).toBeNull();
		expect(g.healthy(after, HEALTHY_BUFFER_S)).toEqual({
			level: 2,
			from: 1,
			reason: 'healthy',
			via: 'load'
		});
		expect(g.healthy(after * 10, FULL)).toBeNull();
	});

	it('climbs one level per window', () => {
		const g = governor();
		g.start();
		g.stall(10_000);
		g.stall(10_000 + STEP_DOWN_COOLDOWN_MS);
		const down = 10_000 + STEP_DOWN_COOLDOWN_MS;
		// Two steps down: the second window applies to every climb.
		const window = HEALTHY_WINDOWS_MS[1];
		expect(g.healthy(down + window, FULL)?.level).toBe(1);
		expect(g.healthy(down + window + window - 1, FULL)).toBeNull();
		expect(g.healthy(down + 2 * window, FULL)?.level).toBe(2);
	});

	it('waits longer after every step down, up to the last window', () => {
		const g = governor();
		g.start();
		let t = 10_000;
		for (let i = 0; i < HEALTHY_WINDOWS_MS.length + 2; i++) {
			expect(g.stall(t)?.level).toBe(1);
			const window = HEALTHY_WINDOWS_MS[Math.min(i, HEALTHY_WINDOWS_MS.length - 1)];
			expect(g.healthy(t + window - 1, FULL)).toBeNull();
			expect(g.healthy(t + window, FULL)?.level).toBe(2);
			t += window + STEP_DOWN_COOLDOWN_MS;
		}
	});

	it('a stall that cannot step down still restarts the healthy window', () => {
		// At the lowest level a stall moves nothing, but playback is in
		// trouble: the climb waits for a window without one.
		const g = governor();
		g.start();
		g.stall(10_000);
		g.stall(10_000 + STEP_DOWN_COOLDOWN_MS);
		expect(g.level).toBe(0);
		expect(g.stall(100_000)).toBeNull();
		const window = HEALTHY_WINDOWS_MS[1];
		expect(g.healthy(100_000 + window - 1, FULL)).toBeNull();
		expect(g.healthy(100_000 + window, FULL)?.level).toBe(1);
	});

	it('a level failing, or a switch followed, restarts the healthy window for the level moved to', () => {
		const failed = governor();
		failed.start();
		failed.stall(10_000);
		expect(failed.levelFailed(1, 100_000)?.level).toBe(0);
		expect(failed.healthy(100_000 + HEALTHY_WINDOWS_MS[0] - 1, FULL)).toBeNull();
		expect(failed.healthy(100_000 + HEALTHY_WINDOWS_MS[0], FULL)?.level).toBe(2);
		const followed = governor();
		followed.start();
		followed.stall(10_000);
		expect(followed.follow(0, 100_000)?.level).toBe(0);
		expect(followed.healthy(100_000 + HEALTHY_WINDOWS_MS[0] - 1, FULL)).toBeNull();
		expect(followed.healthy(100_000 + HEALTHY_WINDOWS_MS[0], FULL)?.level).toBe(2);
	});

	it('a stall inside the cooldown restarts the healthy window too', () => {
		const g = governor();
		g.start();
		g.stall(10_000);
		expect(g.stall(12_000)).toBeNull();
		expect(g.healthy(10_000 + HEALTHY_WINDOWS_MS[0], FULL)).toBeNull();
		expect(g.healthy(12_000 + HEALTHY_WINDOWS_MS[0], FULL)?.level).toBe(2);
	});

	it('never climbs past the level the setting names', () => {
		const g = governor({ kind: 'height', height: 720 });
		g.start();
		g.stall(10_000);
		expect(g.healthy(10_000 + HEALTHY_WINDOWS_MS[0], FULL)?.level).toBe(1);
		expect(g.healthy(10_000_000, FULL)).toBeNull();
		expect(g.level).toBe(1);
	});

	it('a pinned level does not climb', () => {
		const g = governor();
		g.start();
		g.stall(10_000);
		g.pick(1, 11_000);
		expect(g.healthy(10_000_000, FULL)).toBeNull();
	});
});

describe('QualityGovernor — levels the engine cannot load', () => {
	it('a failed playlist moves down and is left for the stream', () => {
		const g = governor();
		g.start();
		expect(g.levelFailed(2)).toEqual({ level: 1, from: 2, reason: 'level-failed', via: 'load' });
		expect(g.unavailableLevels).toEqual([2]);
		g.stall(10_000);
		expect(g.healthy(10_000_000, FULL)?.level).toBe(1);
		expect(g.healthy(20_000_000, FULL)).toBeNull();
		expect(g.pick(2, 30_000_000)).toBeNull();
		expect(g.isPinned).toBe(false);
	});

	it('the lowest level failing moves up instead', () => {
		const g = governor({ kind: 'worst' });
		g.start();
		expect(g.levelFailed(0)?.level).toBe(1);
	});

	it('a level failing that is not the one held only records it', () => {
		const g = governor();
		g.start();
		expect(g.levelFailed(0)).toBeNull();
		expect(g.level).toBe(2);
		expect(g.unavailableLevels).toEqual([0]);
	});

	it('every level failing hands the choice back to the engine', () => {
		const g = governor();
		g.start();
		g.levelFailed(2);
		g.levelFailed(1);
		expect(g.levelFailed(0)).toBeNull();
		expect(g.isEngaged).toBe(false);
		expect(g.stall(10_000_000)).toBeNull();
		expect(g.healthy(20_000_000, FULL)).toBeNull();
	});

	it('segments failing step down at once, pinned or not', () => {
		const g = governor(BEST, 1080);
		g.start();
		g.seeking(1_000);
		expect(g.fragmentsFailed(2, 1_000)).toEqual({
			level: 1,
			from: 2,
			reason: 'fragments-failed',
			via: 'load'
		});
		expect(g.isPinned).toBe(true);
		expect(g.fragmentsFailed(1, 1_001)?.level).toBe(0);
	});

	it('segments of a level no longer held failing change nothing', () => {
		const g = governor();
		g.start();
		g.stall(10_000);
		expect(g.fragmentsFailed(2, 10_500)).toBeNull();
		expect(g.level).toBe(1);
	});

	it('segments of the lowest level failing hand the choice back to the engine', () => {
		const g = governor({ kind: 'worst' });
		g.start();
		expect(g.fragmentsFailed(0, 1_000)).toBeNull();
		expect(g.isEngaged).toBe(false);
	});
});

describe('QualityGovernor — the viewer chooses', () => {
	it('a pick pins the level and shows at once', () => {
		const g = governor();
		g.start();
		expect(g.pick(1, 5_000)).toEqual({ level: 1, from: 2, reason: 'pick', via: 'next' });
		expect(g.isPinned).toBe(true);
	});

	it('picking the level already held pins it without a change', () => {
		const g = governor();
		g.start();
		expect(g.pick(2, 5_000)).toBeNull();
		expect(g.isPinned).toBe(true);
	});

	it('a level that does not exist is no pick', () => {
		const g = governor();
		g.start();
		expect(g.pick(7, 5_000)).toBeNull();
		expect(g.pick(-1, 5_000)).toBeNull();
		expect(g.pick(1.5, 5_000)).toBeNull();
		expect(g.isPinned).toBe(false);
	});

	it('auto hands the choice back, at the setting level, with a fresh window', () => {
		const g = governor();
		g.start();
		g.pick(0, 5_000);
		expect(g.pick('auto', 6_000)).toEqual({ level: 2, from: 0, reason: 'auto', via: 'next' });
		expect(g.isPinned).toBe(false);
		// The stall after it is a new burst: the first window again.
		g.stall(20_000);
		expect(g.healthy(20_000 + HEALTHY_WINDOWS_MS[0], FULL)?.level).toBe(2);
	});

	it('auto at the level already held changes nothing', () => {
		const g = governor();
		g.start();
		expect(g.pick('auto', 5_000)).toBeNull();
	});

	it('auto with the setting level failed takes the highest level left below it', () => {
		const g = governor();
		g.start();
		g.levelFailed(2);
		g.pick(0, 5_000);
		expect(g.pick('auto', 6_000)?.level).toBe(1);
	});

	it('auto with every level up to the setting failed takes the lowest left above it', () => {
		const g = governor({ kind: 'worst' });
		g.start();
		g.levelFailed(0);
		g.pick(2, 5_000);
		expect(g.pick('auto', 6_000)?.level).toBe(1);
	});

	it('auto with every level failed holds nothing', () => {
		const g = governor();
		g.start();
		g.levelFailed(2);
		g.levelFailed(1);
		g.levelFailed(0);
		expect(g.pick('auto', 5_000)).toBeNull();
		expect(g.isEngaged).toBe(false);
		expect(g.isPinned).toBe(false);
	});

	it('a pick takes the choice back from the engine', () => {
		const g = governor();
		g.start();
		g.levelFailed(2);
		g.levelFailed(1);
		g.levelFailed(0);
		expect(g.pick(1, 5_000)).toBeNull();
		expect(g.isEngaged).toBe(false);
		const h = governor({ kind: 'worst' });
		h.start();
		h.fragmentsFailed(0, 1_000);
		expect(h.pick(1, 5_000)?.level).toBe(1);
		expect(h.isEngaged).toBe(true);
	});
});

describe('QualityGovernor — a pin is never exceeded while it is shown', () => {
	it('a pinned lowest level whose segments fail releases the choice and the pin with it', () => {
		const g = governor(BEST, 360);
		g.start();
		expect(g.fragmentsFailed(0, 1_000)).toBeNull();
		expect(g.isEngaged).toBe(false);
		expect(g.isPinned).toBe(false);
	});

	it('a pinned lowest level whose playlist fails moves up only as Auto', () => {
		const g = governor(BEST, 360);
		g.start();
		expect(g.levelFailed(0, 1_000)?.level).toBe(1);
		expect(g.isPinned).toBe(false);
		// Below the pin, the pin holds.
		const h = governor(BEST, 720);
		h.start();
		expect(h.levelFailed(1, 1_000)?.level).toBe(0);
		expect(h.isPinned).toBe(true);
	});

	it('every level failing releases the pin too', () => {
		const g = governor(BEST, 720);
		g.start();
		g.levelFailed(2, 1_000);
		g.levelFailed(0, 1_000);
		expect(g.levelFailed(1, 1_000)).toBeNull();
		expect(g.isPinned).toBe(false);
	});

	it('a switch followed above the pin shows as Auto; one below keeps the pin', () => {
		const up = governor(BEST, 720);
		up.start();
		expect(up.follow(2, 1_000)?.level).toBe(2);
		expect(up.isPinned).toBe(false);
		const down = governor(BEST, 720);
		down.start();
		expect(down.follow(0, 1_000)?.level).toBe(0);
		expect(down.isPinned).toBe(true);
	});
});

describe('QualityGovernor — a pinned play climbs back to its pin', () => {
	it('after its segments failed, climbs back to the pinned level once healthy, and no higher', () => {
		// The setting allows 1080p; the viewer pinned 720p.
		const g = governor(BEST, 720);
		g.start();
		expect(g.fragmentsFailed(1, 1_000)?.level).toBe(0);
		expect(g.isPinned).toBe(true);
		// A stall under a pin steps nothing further down.
		expect(g.stall(5_000)).toBeNull();
		expect(g.healthy(1_000 + HEALTHY_WINDOWS_MS[0] - 1, FULL)).toBeNull();
		expect(g.healthy(1_000 + HEALTHY_WINDOWS_MS[0], FULL)).toEqual({
			level: 1,
			from: 0,
			reason: 'healthy',
			via: 'load'
		});
		expect(g.healthy(10_000_000, FULL)).toBeNull();
		expect(g.isPinned).toBe(true);
	});

	it('waits longer after every fall from the pin, like Auto', () => {
		const g = governor(BEST, 1080);
		g.start();
		g.fragmentsFailed(2, 1_000);
		g.healthy(1_000 + HEALTHY_WINDOWS_MS[0], FULL);
		g.fragmentsFailed(2, 100_000);
		expect(g.healthy(100_000 + HEALTHY_WINDOWS_MS[1] - 1, FULL)).toBeNull();
		expect(g.healthy(100_000 + HEALTHY_WINDOWS_MS[1], FULL)?.level).toBe(2);
	});

	it('picking the pinned level again while below it goes back to it at once', () => {
		const g = governor(BEST, 1080);
		g.start();
		g.fragmentsFailed(2, 1_000);
		expect(g.pick(2, 2_000)).toEqual({ level: 2, from: 1, reason: 'pick', via: 'next' });
	});

	it('a pinned level whose playlist fails while the play is below it moves the pin down to the nearest loadable level', () => {
		const g = governor(BEST, 1080);
		g.start();
		g.fragmentsFailed(2, 1_000);
		expect(g.levelFailed(2, 2_000)).toBeNull();
		expect(g.isPinned).toBe(true);
		// 720p is the pin now: nothing to climb to.
		expect(g.healthy(10_000_000, FULL)).toBeNull();
		// With nothing loadable at or below the pin, the pin is let go.
		const h = governor(BEST, 720);
		h.start();
		h.fragmentsFailed(1, 1_000);
		h.levelFailed(1, 2_000);
		expect(h.isPinned).toBe(true);
		expect(h.levelFailed(0, 3_000)?.level).toBe(2);
		expect(h.isPinned).toBe(false);
	});

	it('the pinned level left over its audio moves the pin to the level the engine chose', () => {
		// hls.js skipped every level between: they share the failing
		// audio. A climb to them would fail and fall again.
		const g = governor(BEST, 1080);
		g.start();
		expect(g.follow(0, 1_000)?.level).toBe(0);
		expect(g.pinnedLevel).toBe(0);
		expect(g.healthy(10_000_000, FULL)).toBeNull();
		// The viewer may still ask for a level between, explicitly.
		expect(g.pick(2, 2_000)).toBeNull();
		expect(g.pick(1, 2_000)?.level).toBe(1);
	});

	it('a play already below its pin that the engine moves further down climbs back to the pin', () => {
		const g = governor(BEST, 1080);
		g.start();
		g.fragmentsFailed(2, 1_000);
		expect(g.follow(0, 2_000)?.level).toBe(0);
		expect(g.pinnedLevel).toBe(2);
		// Two falls: the second window. 720p, left over its audio, is
		// skipped; the climb goes straight to the pin.
		expect(g.healthy(2_000 + HEALTHY_WINDOWS_MS[1] - 1, FULL)).toBeNull();
		expect(g.healthy(2_000 + HEALTHY_WINDOWS_MS[1], FULL)?.level).toBe(2);
	});

	it('a pin re-seated by a removal keeps its place when a switch it is not blamed for follows', () => {
		// The pinned 720p was removed; the pin re-seated on 480p, which
		// never played. The removal's own error then switches to 240p:
		// nothing judged 480p, so the pin stays there and the play
		// climbs back to it.
		const ladder = [
			{ height: 240, bitrate: 400_000 },
			{ height: 360, bitrate: 800_000 },
			{ height: 480, bitrate: 1_400_000 },
			{ height: 720, bitrate: 2_500_000 }
		];
		const g = new QualityGovernor(ladder, BEST, 720);
		g.start();
		g.remap([0, 1, 2, -1], ladder.slice(0, 3));
		expect(g.level).toBe(2);
		expect(g.pinnedLevel).toBe(2);
		expect(g.follow(0, 1_000, false)?.level).toBe(0);
		expect(g.pinnedLevel).toBe(2);
		// One level per window, up to the pin.
		const t1 = 1_000 + HEALTHY_WINDOWS_MS[0];
		expect(g.healthy(t1, FULL)?.level).toBe(1);
		expect(g.healthy(t1 + HEALTHY_WINDOWS_MS[0], FULL)?.level).toBe(2);
	});

	it('an Auto play the engine moved down over its audio does not climb', () => {
		const g = governor();
		g.start();
		g.follow(0, 1_000);
		expect(g.healthy(10_000_000, FULL)).toBeNull();
	});

	it("a play below its pin whose own level's playlist fails moves up only as far as the pin", () => {
		const g = governor(BEST, 1080);
		g.start();
		g.fragmentsFailed(2, 1_000);
		g.levelFailed(0, 1_500);
		// 720p fails: 1080p, the pin, is the nearest loadable level, and the pin holds.
		expect(g.levelFailed(1, 2_000)?.level).toBe(2);
		expect(g.isPinned).toBe(true);
	});
});

describe('QualityGovernor — whatever happens', () => {
	type Step =
		| { k: 'stall'; dt: number }
		| { k: 'seek'; dt: number }
		| { k: 'healthy'; dt: number; buf: number }
		| { k: 'frag'; level: number; dt: number }
		| { k: 'level'; level: number }
		| { k: 'pick'; choice: 'auto' | number; dt: number };
	const step: fc.Arbitrary<Step> = fc.oneof(
		fc.record({ k: fc.constant('stall' as const), dt: fc.integer({ min: 0, max: 60_000 }) }),
		fc.record({ k: fc.constant('seek' as const), dt: fc.integer({ min: 0, max: 60_000 }) }),
		fc.record({
			k: fc.constant('healthy' as const),
			dt: fc.integer({ min: 0, max: 300_000 }),
			buf: fc.integer({ min: 0, max: 60 })
		}),
		fc.record({
			k: fc.constant('frag' as const),
			level: fc.integer({ min: 0, max: 3 }),
			dt: fc.integer({ min: 0, max: 60_000 })
		}),
		fc.record({ k: fc.constant('level' as const), level: fc.integer({ min: 0, max: 3 }) }),
		fc.record({
			k: fc.constant('pick' as const),
			choice: fc.oneof(fc.constant('auto' as const), fc.integer({ min: -1, max: 4 })),
			dt: fc.integer({ min: 0, max: 60_000 })
		})
	);
	const levels: LadderLevel[] = [
		{ height: 360, bitrate: 800_000 },
		{ height: 480, bitrate: 1_400_000 },
		{ height: 720, bitrate: 2_500_000 },
		{ height: 1080, bitrate: 5_000_000 }
	];
	const target: fc.Arbitrary<QualityTarget> = fc.oneof(
		fc.constant({ kind: 'best' } as const),
		fc.constant({ kind: 'worst' } as const),
		fc.integer({ min: 200, max: 1200 }).map((height) => ({ kind: 'height' as const, height }))
	);

	it('holds a real level, never a failed one, and steps down no faster than the cooldown', () => {
		fc.assert(
			fc.property(target, fc.array(step, { maxLength: 60 }), (t, steps) => {
				const g = new QualityGovernor(levels, t, null);
				g.start();
				let now = 0;
				let lastStallStep = -Infinity;
				for (const s of steps) {
					now += 'dt' in s ? s.dt : 0;
					const before = g.level;
					let move = null;
					if (s.k === 'stall') move = g.stall(now);
					else if (s.k === 'seek') g.seeking(now);
					else if (s.k === 'healthy') move = g.healthy(now, s.buf);
					else if (s.k === 'frag') move = g.fragmentsFailed(s.level, now);
					else if (s.k === 'level') move = g.levelFailed(s.level);
					else move = g.pick(s.choice, now);
					if (move) {
						expect(move.from).toBe(before);
						expect(move.level).toBe(g.level);
						expect(move.level).not.toBe(before);
					}
					if (move?.reason === 'stall') {
						expect(now - lastStallStep).toBeGreaterThanOrEqual(STEP_DOWN_COOLDOWN_MS);
						lastStallStep = now;
					}
					if (g.isEngaged) {
						expect(g.level).toBeGreaterThanOrEqual(0);
						expect(g.level).toBeLessThan(levels.length);
						expect(g.unavailableLevels).not.toContain(g.level);
					}
				}
			})
		);
	});
});
