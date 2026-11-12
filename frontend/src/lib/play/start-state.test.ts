import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	chromeReadsPlaying,
	nextStartPhase,
	type StartEvent,
	type StartPhase
} from './start-state';

const at = (paused: boolean, readyState = 0) => ({ paused, readyState });

describe('nextStartPhase', () => {
	it('a stream attached to an autoplaying element is starting, though the element still reads paused', () => {
		expect(nextStartPhase('idle', { kind: 'attach', autoplay: true })).toBe('starting');
	});

	it('a stream attached without autoplay waits paused', () => {
		expect(nextStartPhase('playing', { kind: 'attach', autoplay: false })).toBe('paused');
	});

	it('the first frame playing ends the start', () => {
		expect(nextStartPhase('starting', { kind: 'playing' })).toBe('playing');
	});

	it('waiting before the first frame is still the start, not a stall', () => {
		expect(nextStartPhase('starting', { kind: 'waiting' })).toBe('starting');
	});

	it('waiting while playing is a stall, and playing again ends it', () => {
		const stalled = nextStartPhase('playing', { kind: 'waiting' });
		expect(stalled).toBe('stalled');
		expect(nextStartPhase(stalled, { kind: 'playing' })).toBe('playing');
	});

	it('a seek while playing stalls until it lands with data', () => {
		const s = nextStartPhase('playing', { kind: 'seeking', ...at(false, 1) });
		expect(s).toBe('stalled');
		expect(nextStartPhase(s, { kind: 'seeked', ...at(false, 4) })).toBe('playing');
		expect(nextStartPhase(s, { kind: 'seeked', ...at(false, 1) })).toBe('stalled');
	});

	it('a seek while paused stays paused', () => {
		expect(nextStartPhase('paused', { kind: 'seeking', ...at(true, 1) })).toBe('paused');
	});

	it('a viewer pause during the start is a pause', () => {
		expect(nextStartPhase('starting', { kind: 'pause' })).toBe('paused');
	});

	it('play from a pause starts again until frames play', () => {
		expect(nextStartPhase('paused', { kind: 'play' })).toBe('starting');
	});

	it('autoplay refused: data enough and still paused reads paused', () => {
		expect(nextStartPhase('starting', { kind: 'canplay', ...at(true, 4) })).toBe('paused');
		expect(nextStartPhase('starting', { kind: 'canplay', ...at(true, 2) })).toBe('starting');
	});

	it('an error or a detach ends any phase', () => {
		expect(nextStartPhase('starting', { kind: 'error' })).toBe('idle');
		expect(nextStartPhase('stalled', { kind: 'detach' })).toBe('idle');
	});

	it('the end of the episode is ended, and play from there starts again', () => {
		const e = nextStartPhase('playing', { kind: 'ended' });
		expect(e).toBe('ended');
		expect(nextStartPhase(e, { kind: 'play' })).toBe('starting');
	});
});

describe('chromeReadsPlaying', () => {
	it("reads the viewer's intent: a start and a stall show the pause glyph, not Play", () => {
		expect(chromeReadsPlaying('starting')).toBe(true);
		expect(chromeReadsPlaying('stalled')).toBe(true);
		expect(chromeReadsPlaying('playing')).toBe(true);
		expect(chromeReadsPlaying('paused')).toBe(false);
		expect(chromeReadsPlaying('ended')).toBe(false);
		expect(chromeReadsPlaying('idle')).toBe(false);
	});
});

const phases: StartPhase[] = ['idle', 'starting', 'playing', 'stalled', 'paused', 'ended'];
const eventArb: fc.Arbitrary<StartEvent> = fc.oneof(
	fc.boolean().map((autoplay) => ({ kind: 'attach' as const, autoplay })),
	fc.constantFrom(
		{ kind: 'play' as const },
		{ kind: 'playing' as const },
		{ kind: 'waiting' as const },
		{ kind: 'pause' as const },
		{ kind: 'ended' as const },
		{ kind: 'error' as const },
		{ kind: 'detach' as const }
	),
	fc.record({
		kind: fc.constantFrom('seeking' as const, 'seeked' as const, 'canplay' as const),
		paused: fc.boolean(),
		readyState: fc.integer({ min: 0, max: 4 })
	})
);

describe('nextStartPhase properties', () => {
	it('is total and stays inside the phase set', () => {
		fc.assert(
			fc.property(fc.constantFrom(...phases), eventArb, (p, e) => {
				expect(phases).toContain(nextStartPhase(p, e));
			})
		);
	});

	it('the playing event always ends a start or a stall; a pause always reads paused', () => {
		fc.assert(
			fc.property(fc.array(eventArb, { maxLength: 30 }), (events) => {
				let p: StartPhase = 'idle';
				for (const e of events) p = nextStartPhase(p, e);
				expect(nextStartPhase(p, { kind: 'playing' })).toBe('playing');
				expect(nextStartPhase(p, { kind: 'pause' })).toBe('paused');
			})
		);
	});
});
