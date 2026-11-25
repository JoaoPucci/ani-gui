import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SHOW_AFTER_MS } from './load-tip';
import { LoadTipSession } from './load-tip-session.svelte';

let s: LoadTipSession;
let seen: number;
let gone: number;

beforeEach(() => {
	vi.useFakeTimers();
	seen = 0;
	gone = 0;
	s = new LoadTipSession({
		now: () => Date.now(),
		setTimer: (fn, ms) => setTimeout(fn, ms),
		clearTimer: (t) => clearTimeout(t),
		onSeen: () => seen++,
		onGone: () => gone++
	});
});
afterEach(() => {
	s.stop();
	vi.useRealTimers();
});

describe('LoadTipSession', () => {
	it('shows once a start has lasted SHOW_AFTER_MS, with nothing more fed', () => {
		s.feed({ veil: 'start', blocked: false, fresh: true });
		vi.advanceTimersByTime(SHOW_AFTER_MS - 1);
		expect(s.up).toBe(false);
		vi.advanceTimersByTime(1);
		expect(s.state.phase).toBe('loading');
	});

	it('a load that lands sooner shows nothing', () => {
		s.feed({ veil: 'start', blocked: false, fresh: true });
		vi.advanceTimersByTime(400);
		s.feed({ veil: 'off', blocked: false, fresh: true });
		vi.advanceTimersByTime(SHOW_AFTER_MS * 2);
		expect(s.up).toBe(false);
	});

	it('stays folded once playback starts, and a close is seen once', () => {
		s.feed({ veil: 'start', blocked: false, fresh: true });
		vi.advanceTimersByTime(SHOW_AFTER_MS);
		s.feed({ veil: 'off', blocked: false, fresh: true });
		expect(s.state.phase).toBe('playing');
		s.close();
		s.close();
		expect(seen).toBe(1);
		expect(gone).toBe(1);
	});

	it('an error while up takes it away unseen', () => {
		s.feed({ veil: 'start', blocked: false, fresh: true });
		vi.advanceTimersByTime(SHOW_AFTER_MS);
		s.feed({ veil: 'start', blocked: true, fresh: true });
		expect(s.state).toMatchObject({ phase: 'gone', seen: false });
		expect(seen).toBe(0);
		expect(gone).toBe(1);
	});

	it('a stop ends the wait', () => {
		s.feed({ veil: 'start', blocked: false, fresh: true });
		s.stop();
		vi.advanceTimersByTime(SHOW_AFTER_MS * 2);
		expect(s.up).toBe(false);
	});

	it('a timer that fires a hair early looks again, and shows it', () => {
		let clock = 0;
		const timers: { fn: () => void; ms: number }[] = [];
		const early = new LoadTipSession({
			now: () => clock,
			setTimer: (fn, ms) => {
				timers.push({ fn, ms });
				return 0 as unknown as ReturnType<typeof setTimeout>;
			},
			clearTimer: () => {},
			onSeen: () => {},
			onGone: () => {}
		});
		early.feed({ veil: 'start', blocked: false, fresh: true });
		clock = SHOW_AFTER_MS - 1;
		timers.shift()!.fn();
		expect(early.state.phase).toBe('waiting');
		expect(timers).toHaveLength(1);
		clock = SHOW_AFTER_MS;
		timers.shift()!.fn();
		expect(early.state.phase).toBe('loading');
	});

	it('a new source brings a tip that went unseen back to waiting, and leaves a seen one gone', () => {
		s.feed({ veil: 'start', blocked: false, fresh: true });
		vi.advanceTimersByTime(SHOW_AFTER_MS);
		s.feed({ veil: 'start', blocked: true, fresh: true });
		s.newSource();
		expect(s.state.phase).toBe('idle');
		s.feed({ veil: 'start', blocked: false, fresh: true });
		vi.advanceTimersByTime(SHOW_AFTER_MS);
		s.close();
		s.newSource();
		expect(s.state).toMatchObject({ phase: 'gone', seen: true });
	});
});
