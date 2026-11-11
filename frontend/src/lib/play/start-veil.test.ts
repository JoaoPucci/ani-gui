import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { clock, episodeNumeral, veilMode, type VeilInput } from './start-veil';
import type { StartPhase } from './start-state';

const base: VeilInput = {
	phase: 'playing',
	resumeHolding: false,
	switchBusy: false,
	fullscreen: false,
	videoFullscreen: false,
	playerError: false,
	customControls: true
};

describe('veilMode', () => {
	it('a start and a stall show the veil with the custom controls', () => {
		expect(veilMode({ ...base, phase: 'starting' })).toBe('start');
		expect(veilMode({ ...base, phase: 'stalled' })).toBe('stall');
		expect(veilMode({ ...base, phase: 'playing' })).toBe('off');
		expect(veilMode({ ...base, phase: 'paused' })).toBe('off');
	});

	it("with Chromium's own controls a start or a stall is theirs, but a resume hold is ours", () => {
		const native = { ...base, customControls: false };
		expect(veilMode({ ...native, phase: 'starting' })).toBe('off');
		expect(veilMode({ ...native, phase: 'stalled' })).toBe('off');
		expect(veilMode({ ...native, phase: 'paused', resumeHolding: true })).toBe('start');
	});

	it('a resume hold is a start, whatever the phase reads', () => {
		expect(veilMode({ ...base, phase: 'paused', resumeHolding: true })).toBe('start');
	});

	it('a switch shows the next episode in fullscreen; windowed, the page overlay has it', () => {
		expect(veilMode({ ...base, switchBusy: true, fullscreen: true })).toBe('start');
		expect(veilMode({ ...base, switchBusy: true })).toBe('off');
	});

	it('an error, or a video fullscreen on its own, shows no veil', () => {
		expect(veilMode({ ...base, phase: 'starting', playerError: true })).toBe('off');
		expect(veilMode({ ...base, resumeHolding: true, videoFullscreen: true })).toBe('off');
	});

	it('is total: an error always wins, and a playing or paused stream with nothing else on shows nothing', () => {
		const phases: StartPhase[] = ['idle', 'starting', 'playing', 'stalled', 'paused', 'ended'];
		fc.assert(
			fc.property(
				fc.record({
					phase: fc.constantFrom(...phases),
					resumeHolding: fc.boolean(),
					switchBusy: fc.boolean(),
					fullscreen: fc.boolean(),
					videoFullscreen: fc.boolean(),
					playerError: fc.boolean(),
					customControls: fc.boolean()
				}),
				(input) => {
					const mode = veilMode(input);
					expect(['off', 'start', 'stall']).toContain(mode);
					if (input.playerError || input.videoFullscreen) expect(mode).toBe('off');
					if (
						!input.resumeHolding &&
						!input.switchBusy &&
						input.phase !== 'starting' &&
						input.phase !== 'stalled'
					)
						expect(mode).toBe('off');
				}
			)
		);
	});
});

describe('clock', () => {
	it('reads m:ss under an hour and h:mm:ss from there', () => {
		expect(clock(0)).toBe('0:00');
		expect(clock(400.7)).toBe('6:40');
		expect(clock(3599)).toBe('59:59');
		expect(clock(3600)).toBe('1:00:00');
		expect(clock(-5)).toBe('0:00');
		expect(clock(Number.NaN)).toBe('0:00');
	});

	it('round-trips whole seconds', () => {
		fc.assert(
			fc.property(fc.integer({ min: 0, max: 99 * 3600 }), (s) => {
				const parts = clock(s).split(':').map(Number);
				const back = parts.reduce((acc, p) => acc * 60 + p, 0);
				expect(back).toBe(s);
			})
		);
	});
});

describe('episodeNumeral', () => {
	it('pads a whole episode under ten to two digits, and leaves the rest as they are', () => {
		expect(episodeNumeral(0)).toBe('00');
		expect(episodeNumeral(3)).toBe('03');
		expect(episodeNumeral(12)).toBe('12');
		expect(episodeNumeral(367)).toBe('367');
		expect(episodeNumeral(7.5)).toBe('7.5');
	});

	it('always names the same number', () => {
		fc.assert(
			fc.property(fc.integer({ min: 0, max: 5000 }), (n) => {
				expect(Number(episodeNumeral(n))).toBe(n);
			})
		);
	});
});
