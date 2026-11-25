import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	LOAD_TIP_IDLE,
	SHOW_AFTER_MS,
	loadTipClose,
	loadTipStep,
	loadTipUp,
	type LoadTipInput,
	type LoadTipState
} from './load-tip';

const run = (inputs: LoadTipInput[], from: LoadTipState = LOAD_TIP_IDLE) =>
	inputs.reduce(loadTipStep, from);
const at = (now: number, veil: LoadTipInput['veil'] = 'start', blocked = false): LoadTipInput => ({
	veil,
	blocked,
	now
});

describe('loadTipStep', () => {
	it('shows once a start has lasted SHOW_AFTER_MS', () => {
		expect(run([at(0), at(SHOW_AFTER_MS - 1)]).phase).toBe('waiting');
		expect(run([at(0), at(SHOW_AFTER_MS)]).phase).toBe('loading');
	});

	it('a fast load never shows it, and it waits for a later slow one', () => {
		const fast = run([at(0), at(400, 'off')]);
		expect(fast).toEqual(LOAD_TIP_IDLE);
		expect(run([at(2000), at(2000 + SHOW_AFTER_MS)], fast).phase).toBe('loading');
	});

	it('never starts in a stall, in fullscreen, over an error or a dialog', () => {
		expect(run([at(0, 'stall'), at(5000, 'stall')]).phase).toBe('idle');
		expect(run([at(0, 'start', true), at(5000, 'start', true)]).phase).toBe('idle');
		expect(run([at(0), at(500, 'start', true), at(5000, 'start', true)]).phase).toBe('idle');
	});

	it('stays, folded, when the first frame comes, and a stall leaves it be', () => {
		const up = run([at(0), at(SHOW_AFTER_MS), at(SHOW_AFTER_MS + 200, 'off')]);
		expect(up.phase).toBe('playing');
		expect(loadTipStep(up, at(9000, 'stall')).phase).toBe('playing');
	});

	it('anything that blocks it while it is up takes it away, still pending', () => {
		const loading = run([at(0), at(SHOW_AFTER_MS)]);
		expect(loadTipStep(loading, at(1500, 'start', true))).toMatchObject({
			phase: 'gone',
			seen: false
		});
		const playing = loadTipStep(loading, at(1500, 'off'));
		expect(loadTipStep(playing, at(2000, 'off', true))).toMatchObject({
			phase: 'gone',
			seen: false
		});
	});

	it('closing it is the one way it is seen, and only while it is up', () => {
		const loading = run([at(0), at(SHOW_AFTER_MS)]);
		expect(loadTipClose(loading)).toMatchObject({ phase: 'gone', seen: true });
		expect(loadTipClose(LOAD_TIP_IDLE)).toEqual(LOAD_TIP_IDLE);
		const waiting = run([at(0)]);
		expect(loadTipClose(waiting)).toBe(waiting);
	});

	it('is up only after an unbroken start of SHOW_AFTER_MS, never while blocked, seen only by a close, and gone stays gone (property)', () => {
		const input = fc.record({
			veil: fc.constantFrom<LoadTipInput['veil']>('off', 'start', 'stall'),
			blocked: fc.boolean(),
			dt: fc.integer({ min: 0, max: 800 })
		});
		fc.assert(
			fc.property(fc.array(input, { maxLength: 30 }), (steps) => {
				let s = LOAD_TIP_IDLE;
				let now = 0;
				let run: number | null = null;
				for (const { veil, blocked, dt } of steps) {
					now += dt;
					const was = s;
					s = loadTipStep(s, { veil, blocked, now });
					if (was.phase === 'gone' && s !== was) return false;
					if (s.seen) return false;
					if (loadTipUp(s) && blocked) return false;
					// The unbroken start the tip waits out.
					if (veil === 'start' && !blocked) run ??= now;
					else run = null;
					if (s.phase === 'loading' && !loadTipUp(was) && now - (run ?? now) < SHOW_AFTER_MS)
						return false;
				}
				return true;
			})
		);
	});
});
