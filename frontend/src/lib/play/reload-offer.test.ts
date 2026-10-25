import { describe, expect, it, vi } from 'vitest';
import type { AvailabilityBatchResponse } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { deadEnd, deadEndMessage, watchDeadEndOtherAudio, type DeadEndQuery } from './reload-offer';

const OPEN = { listed: true, unaired: false, beyondPlayable: false };

describe('deadEnd — the episodes a play cannot reach', () => {
	it('is null when nothing known keeps the play from the episode', () => {
		expect(deadEnd(OPEN)).toBeNull();
		// An unanswered listing claims nothing.
		expect(deadEnd({ ...OPEN, listed: null })).toBeNull();
	});

	it('names each refusal', () => {
		expect(deadEnd({ ...OPEN, listed: false })).toBe('delisted');
		expect(deadEnd({ ...OPEN, unaired: true })).toBe('unaired');
		expect(deadEnd({ ...OPEN, beyondPlayable: true })).toBe('beyond-playable');
	});

	it('puts a delisted show ahead of what is said about its episodes', () => {
		// A delisted show keeps whatever cap it had, so its episodes'
		// standing says nothing about why the play is refused.
		expect(deadEnd({ listed: false, unaired: true, beyondPlayable: true })).toBe('delisted');
		expect(deadEnd({ listed: true, unaired: true, beyondPlayable: true })).toBe('unaired');
	});
});

describe('deadEndMessage — the words the rest of the app uses for the state', () => {
	it('a delisted show reads as the detail page notice, naming the other audio only on evidence', () => {
		for (const other of [null, false] as const) {
			expect(deadEndMessage('delisted', 'sub', other)).toBe(m.detail_unavailable_message());
			expect(deadEndMessage('delisted', 'dub', other)).toBe(m.detail_unavailable_message());
		}
		expect(deadEndMessage('delisted', 'dub', true)).toBe(m.play_error_stream_unavailable_in_dub());
		expect(deadEndMessage('delisted', 'sub', true)).toBe(m.play_error_stream_unavailable_in_sub());
	});

	it('an episode past the count reads as the play-failure copy for an episode with no stream', () => {
		expect(deadEndMessage('beyond-playable', 'sub', null)).toBe(
			m.play_play_failure_episode_unavailable()
		);
		expect(deadEndMessage('beyond-playable', 'sub', false)).toBe(
			m.play_play_failure_episode_unavailable()
		);
		expect(deadEndMessage('beyond-playable', 'sub', true)).toBe(
			m.play_play_failure_episode_unavailable_try_other_audio()
		);
	});

	it('an unaired episode says so, whatever the other audio holds', () => {
		for (const other of [null, false, true] as const) {
			expect(deadEndMessage('unaired', 'sub', other)).toBe(m.detail_ep_unaired_tooltip());
		}
	});
});

describe('watchDeadEndOtherAudio — what the cache says, asked only where it matters', () => {
	const Q: DeadEndQuery = { kitsuId: '42', mode: 'sub', episode: 10, music: false, refused: false };
	const HAS = { cached: { '42': true }, playable_episode_counts: { '42': 12 } };

	function batchAnswering(res: AvailabilityBatchResponse) {
		return vi.fn<(ids: string[], mode: string) => Promise<AvailabilityBatchResponse>>(
			async () => res
		);
	}
	const flush = () => new Promise((r) => setTimeout(r, 0));

	it('asks the other audio about the title for a delisted show', async () => {
		const batch = batchAnswering(HAS);
		const onAnswer = vi.fn();
		watchDeadEndOtherAudio('delisted', Q, onAnswer, batch);
		await flush();
		expect(batch).toHaveBeenCalledWith(['42'], 'dub');
		expect(onAnswer).toHaveBeenCalledWith(true);
	});

	it('asks about the episode when it is past the count', async () => {
		const short = batchAnswering({ cached: { '42': true }, playable_episode_counts: { '42': 9 } });
		const onAnswer = vi.fn();
		watchDeadEndOtherAudio('beyond-playable', Q, onAnswer, short);
		await flush();
		expect(short).toHaveBeenCalledWith(['42'], 'dub');
		// The other audio's count stops short of episode 10.
		expect(onAnswer).toHaveBeenCalledWith(false);
	});

	it('asks nothing where the detail page would not, nor for an unaired episode', async () => {
		const batch = batchAnswering(HAS);
		const onAnswer = vi.fn();
		watchDeadEndOtherAudio('delisted', { ...Q, music: true }, onAnswer, batch);
		watchDeadEndOtherAudio('delisted', { ...Q, refused: true }, onAnswer, batch);
		watchDeadEndOtherAudio('unaired', Q, onAnswer, batch);
		await flush();
		expect(batch).not.toHaveBeenCalled();
		expect(onAnswer).not.toHaveBeenCalled();
	});

	it('drops an answer that lands after the cancel', async () => {
		const batch = batchAnswering(HAS);
		const onAnswer = vi.fn();
		const cancel = watchDeadEndOtherAudio('delisted', Q, onAnswer, batch);
		cancel();
		await flush();
		expect(onAnswer).not.toHaveBeenCalled();
	});
});
