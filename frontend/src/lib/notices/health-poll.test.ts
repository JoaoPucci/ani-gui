import { describe, expect, it, vi } from 'vitest';
import { HEALTH_POLL_MS, startHealthPoll } from './health-poll';

const flush = () => new Promise((r) => setTimeout(r, 0));
const BODY = { providers: [{ provider: 'hianime', state: 'answering' }] };

function deps(read: () => Promise<unknown>) {
	const timers: { fn: () => void; ms: number }[] = [];
	const onHealth = vi.fn();
	const clearInterval = vi.fn();
	return {
		timers,
		onHealth,
		clearInterval,
		d: {
			read,
			onHealth,
			setInterval: (fn: () => void, ms: number) => timers.push({ fn, ms }),
			clearInterval
		}
	};
}

describe('reading the source health', () => {
	it('reads at once and then on its interval', async () => {
		const read = vi.fn(async () => BODY);
		const t = deps(read);
		startHealthPoll(t.d);
		expect(t.timers[0].ms).toBe(HEALTH_POLL_MS);
		await flush();
		expect(t.onHealth).toHaveBeenCalledWith(BODY.providers);
		t.timers[0].fn();
		await flush();
		expect(read).toHaveBeenCalledTimes(2);
	});

	it('does not stack reads while one is under way', async () => {
		let answer: (v: unknown) => void = () => {};
		const read = vi.fn(() => new Promise((r) => (answer = r)));
		const t = deps(read);
		const poll = startHealthPoll(t.d);
		poll.poke();
		t.timers[0].fn();
		expect(read).toHaveBeenCalledTimes(1);
		answer(BODY);
		await flush();
		poll.poke();
		expect(read).toHaveBeenCalledTimes(2);
	});

	it('changes nothing when the read fails', async () => {
		const t = deps(async () => {
			throw new Error('offline');
		});
		const poll = startHealthPoll(t.d);
		await flush();
		expect(t.onHealth).not.toHaveBeenCalled();
		poll.poke();
		await flush();
		expect(t.onHealth).not.toHaveBeenCalled();
	});

	it('stops reading, and drops an answer that arrives after it stopped', async () => {
		let answer: (v: unknown) => void = () => {};
		const read = vi.fn(() => new Promise((r) => (answer = r)));
		const t = deps(read);
		const poll = startHealthPoll(t.d);
		poll.stop();
		expect(t.clearInterval).toHaveBeenCalledWith(1);
		answer(BODY);
		await flush();
		expect(t.onHealth).not.toHaveBeenCalled();
		poll.poke();
		expect(read).toHaveBeenCalledTimes(1);
	});
});
