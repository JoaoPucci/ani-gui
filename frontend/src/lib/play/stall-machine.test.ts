import { describe, expect, it } from 'vitest';
import { HlsStallMachine } from './stall-machine';
import { STALL_NUDGE_BUDGET } from './stale-stream';

const hostSlow = {
	err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' } as const,
	hasAutoRetried: false,
	playbackProgressed: true
};

describe('HlsStallMachine', () => {
	it('nudges with a toast opening the burst, silently after', () => {
		const m = new HlsStallMachine();
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: false });
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: false });
	});

	it('escalates once the burst budget is spent', () => {
		const m = new HlsStallMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) m.failure(hostSlow);
		expect(m.failure(hostSlow)).toEqual({ act: 'recover' });
	});

	it('a landed MAIN fragment ends the burst', () => {
		const m = new HlsStallMachine();
		m.failure(hostSlow);
		m.fragmentLoaded({ frag: { type: 'main' } });
		// Fresh burst: toast again, full budget again.
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
	});

	it('side-rendition fragments do not refill the budget', () => {
		// Subtitle and audio renditions keep landing while the video
		// rendition times out; resetting on them would let the burst
		// refill forever and never escalate to the fresh-link recovery.
		const m = new HlsStallMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) {
			m.failure(hostSlow);
			m.fragmentLoaded({ frag: { type: 'audio' } });
			m.fragmentLoaded({ frag: { type: 'subtitle' } });
			m.fragmentLoaded({});
		}
		expect(m.failure(hostSlow)).toEqual({ act: 'recover' });
	});

	it('reset gives the next stream a fresh budget and a fresh toast', () => {
		const m = new HlsStallMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) m.failure(hostSlow);
		m.reset();
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
	});

	it('non-nudge failures pass through the ladder untouched', () => {
		const m = new HlsStallMachine();
		expect(
			m.failure({
				err: { source: 'hls', type: 'networkError', details: 'fragLoadError' },
				hasAutoRetried: false,
				playbackProgressed: true
			})
		).toEqual({ act: 'recover' });
		expect(
			m.failure({
				err: { source: 'hls', type: 'mediaError' },
				hasAutoRetried: true,
				playbackProgressed: true
			})
		).toEqual({ act: 'surface' });
	});
});
