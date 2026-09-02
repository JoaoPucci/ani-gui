import { describe, expect, it } from 'vitest';
import { HlsStallMachine } from './stall-machine';
import { STALL_NUDGE_BUDGET } from './stale-stream';

const hostSlow = {
	err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' } as const,
	hasAutoRetried: false
};

/** A machine whose stream has proven itself — playback ran. */
function progressedMachine(): HlsStallMachine {
	const m = new HlsStallMachine();
	m.progressed();
	return m;
}

describe('HlsStallMachine', () => {
	it('nudges with a toast opening the burst, silently after', () => {
		const m = progressedMachine();
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: false });
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: false });
	});

	it('escalates once the burst budget is spent', () => {
		const m = progressedMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) m.failure(hostSlow);
		expect(m.failure(hostSlow)).toEqual({ act: 'recover' });
	});

	it('a landed MAIN fragment ends the burst', () => {
		const m = progressedMachine();
		m.failure(hostSlow);
		m.fragmentLoaded({ frag: { type: 'main' } });
		// Fresh burst: toast again, full budget again.
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
	});

	it('side-rendition fragments do not refill the budget', () => {
		// Subtitle and audio renditions keep landing while the video
		// rendition times out; resetting on them would let the burst
		// refill forever and never escalate to the fresh-link recovery.
		const m = progressedMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) {
			m.failure(hostSlow);
			m.fragmentLoaded({ frag: { type: 'audio' } });
			m.fragmentLoaded({ frag: { type: 'subtitle' } });
			m.fragmentLoaded({});
		}
		expect(m.failure(hostSlow)).toEqual({ act: 'recover' });
	});

	it('reset gives the next stream a fresh budget and a fresh toast', () => {
		const m = progressedMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) m.failure(hostSlow);
		m.reset();
		expect(m.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
	});

	it('non-nudge failures pass through the ladder untouched', () => {
		const m = progressedMachine();
		expect(
			m.failure({
				err: { source: 'hls', type: 'networkError', details: 'fragLoadError' },
				hasAutoRetried: false
			})
		).toEqual({ act: 'recover' });
		expect(
			m.failure({
				err: { source: 'hls', type: 'mediaError' },
				hasAutoRetried: true
			})
		).toEqual({ act: 'surface' });
	});
});

describe('HlsStallMachine — rendition correlation', () => {
	const audioStall = {
		err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' } as const,
		hasAutoRetried: false,
		rendition: 'audio'
	};

	it('main fragments do not end an audio-rendition stall', () => {
		// The inverse of the side-rendition case: video keeps landing
		// while the audio rendition times out. Those main fragments
		// prove nothing about the stalled rendition — resetting on
		// them would keep the audio burst from ever escalating.
		const m = progressedMachine();
		for (let i = 0; i < STALL_NUDGE_BUDGET; i++) {
			m.failure(audioStall);
			m.fragmentLoaded({ frag: { type: 'main' } });
		}
		expect(m.failure(audioStall)).toEqual({ act: 'recover' });
	});

	it('the stalled rendition landing ends its own burst', () => {
		const m = progressedMachine();
		m.failure(audioStall);
		m.fragmentLoaded({ frag: { type: 'audio' } });
		expect(m.failure(audioStall)).toEqual({ act: 'nudge', toast: true });
	});

	it('a failure without rendition data stalls the main rendition', () => {
		// hls.js fatals do not always carry a frag; the default keeps
		// the common case — the video rendition — correct.
		const m = progressedMachine();
		m.failure({
			err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' },
			hasAutoRetried: false
		});
		m.fragmentLoaded({ frag: { type: 'main' } });
		expect(
			m.failure({
				err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' },
				hasAutoRetried: false
			})
		).toEqual({ act: 'nudge', toast: true });
	});
});

describe('HlsStallMachine — budgets are per rendition', () => {
	const stall = (rendition: string) => ({
		err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' } as const,
		hasAutoRetried: false,
		rendition
	});

	it("one rendition's success cannot replenish another's budget", () => {
		// Audio and main failures interleave; the main rendition
		// recovering clears MAIN's burst only. The audio rendition's
		// accumulated failures survive, so its budget still exhausts
		// and escalates.
		const m = progressedMachine();
		expect(m.failure(stall('audio'))).toEqual({ act: 'nudge', toast: true });
		expect(m.failure(stall('main'))).toEqual({ act: 'nudge', toast: false });
		m.fragmentLoaded({ frag: { type: 'main' } });
		expect(m.failure(stall('audio'))).toEqual({ act: 'nudge', toast: false });
		expect(m.failure(stall('audio'))).toEqual({ act: 'nudge', toast: false });
		expect(m.failure(stall('audio'))).toEqual({ act: 'recover' });
	});

	it('the toast marks the first active stall, not every rendition', () => {
		// One "host is slow" notice per trouble window: a second
		// rendition joining an active stall stays silent, and the
		// toast returns once every burst has cleared.
		const m = progressedMachine();
		expect(m.failure(stall('audio'))).toEqual({ act: 'nudge', toast: true });
		expect(m.failure(stall('main'))).toEqual({ act: 'nudge', toast: false });
		m.fragmentLoaded({ frag: { type: 'main' } });
		m.fragmentLoaded({ frag: { type: 'audio' } });
		expect(m.failure(stall('main'))).toEqual({ act: 'nudge', toast: true });
	});
});

describe('HlsStallMachine — progress is remembered, not derived', () => {
	it('a backward seek does not turn a proven stream into the startup case', () => {
		// The viewer played minutes, then seeked back into the first
		// second. The stream is still proven — a stall must nudge, not
		// take the disruptive evict + re-resolve this path exists to
		// avoid. The machine remembers progress until reset; there is
		// nothing the adapter can pass that forgets it.
		const m = new HlsStallMachine();
		m.progressed();
		expect(
			m.failure({
				err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' },
				hasAutoRetried: false
			})
		).toEqual({ act: 'nudge', toast: true });
	});
});
