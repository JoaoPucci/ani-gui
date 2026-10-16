import { describe, expect, it, vi } from 'vitest';
import type { AvailabilityBatchResponse } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { asksOtherAudio, unavailableMessage, watchOtherAudio } from './unavailable-copy';

function batchAnswering(res: AvailabilityBatchResponse) {
	return vi.fn<(ids: string[], mode: string) => Promise<AvailabilityBatchResponse>>(
		async () => res
	);
}

describe('unavailableMessage — the notice that replaces Play', () => {
	it('names the other audio when the cache has the title in it', () => {
		// Kitsu 40684: dub cached unavailable, sub cached with four
		// episodes. "Not in the streaming catalogue" is false there.
		expect(unavailableMessage('dub', true)).toBe(m.detail_unavailable_in_dub_message());
		expect(unavailableMessage('sub', true)).toBe(m.detail_unavailable_in_sub_message());
		expect(unavailableMessage('dub', true)).not.toBe(m.detail_unavailable_message());
	});

	it('keeps the catalogue sentence when the other audio has nothing or nothing vouches', () => {
		for (const mode of ['sub', 'dub']) {
			expect(unavailableMessage(mode, false)).toBe(m.detail_unavailable_message());
			expect(unavailableMessage(mode, null)).toBe(m.detail_unavailable_message());
		}
	});

	it('says the switch brings back download as well as playback', () => {
		// The notice it replaces says both are unavailable; the switch
		// restores both.
		expect(m.detail_unavailable_in_dub_message()).toMatch(/watch or download/);
		expect(m.detail_unavailable_in_sub_message()).toMatch(/watch or download/);
		const verbs = {
			'pt-BR': /assistir ou baixar/,
			'es-419': /verlo o descargarlo/,
			ru: /посмотреть или скачать/
		};
		for (const [locale, verb] of Object.entries(verbs) as ['pt-BR' | 'es-419' | 'ru', RegExp][]) {
			expect(m.detail_unavailable_in_dub_message({}, { locale }), locale).toMatch(verb);
			expect(m.detail_unavailable_in_sub_message({}, { locale }), locale).toMatch(verb);
		}
	});

	it('every locale names the audio that has the title, translated', () => {
		for (const locale of ['pt-BR', 'es-419', 'ru'] as const) {
			const dub = m.detail_unavailable_in_dub_message({}, { locale });
			const sub = m.detail_unavailable_in_sub_message({}, { locale });
			expect(dub, locale).toContain('SUB');
			expect(sub, locale).toContain('DUB');
			expect(dub, locale).not.toBe(m.detail_unavailable_in_dub_message({}, { locale: 'en' }));
			expect(sub, locale).not.toBe(m.detail_unavailable_in_sub_message({}, { locale: 'en' }));
		}
	});
});

describe('watchOtherAudio — the notice asks the cache, never the provider', () => {
	it('asks the cache-only batch route for the other audio, show-level', async () => {
		// Show-level: a count in the cache does not narrow it, since
		// no episode is in question on the notice.
		const batch = batchAnswering({
			cached: { '40684': true },
			playable_episode_counts: { '40684': 4 }
		});
		const onAnswer = vi.fn();
		watchOtherAudio({ kitsuId: '40684', mode: 'dub' }, onAnswer, batch);
		await vi.waitFor(() => expect(onAnswer).toHaveBeenCalledWith(true));
		expect(batch).toHaveBeenCalledWith(['40684'], 'sub');
	});

	it('takes a count-less positive row as the show being there', async () => {
		// The notice's claim is show-level, and a positive row proves
		// the show — a dub resolve stamps one without a count.
		const onAnswer = vi.fn();
		watchOtherAudio(
			{ kitsuId: '7', mode: 'sub' },
			onAnswer,
			batchAnswering({ cached: { '7': true }, playable_episode_counts: {} })
		);
		await vi.waitFor(() => expect(onAnswer).toHaveBeenCalledWith(true));
	});

	it('does not claim a show whose cached count is zero streams there', async () => {
		const onAnswer = vi.fn();
		watchOtherAudio(
			{ kitsuId: '7', mode: 'sub' },
			onAnswer,
			batchAnswering({ cached: { '7': true }, playable_episode_counts: { '7': 0 } })
		);
		await vi.waitFor(() => expect(onAnswer).toHaveBeenCalledWith(null));
	});

	it('is unknown when the lookup fails', async () => {
		const onAnswer = vi.fn();
		watchOtherAudio({ kitsuId: '7', mode: 'sub' }, onAnswer, async () => {
			throw new Error('sidecar gone');
		});
		await vi.waitFor(() => expect(onAnswer).toHaveBeenCalledWith(null));
	});

	it('hands over false and unknown as they come', async () => {
		const no = vi.fn();
		watchOtherAudio(
			{ kitsuId: '7', mode: 'sub' },
			no,
			batchAnswering({ cached: { '7': false }, playable_episode_counts: {} })
		);
		await vi.waitFor(() => expect(no).toHaveBeenCalledWith(false));
		const unknown = vi.fn();
		watchOtherAudio(
			{ kitsuId: '7', mode: 'sub' },
			unknown,
			batchAnswering({ cached: {}, playable_episode_counts: {} })
		);
		await vi.waitFor(() => expect(unknown).toHaveBeenCalledWith(null));
	});

	it('drops an answer that lands after it was cancelled', async () => {
		// The audio or the title changed while the lookup was out; the
		// late answer is about a question nobody is asking any more.
		let release!: (r: AvailabilityBatchResponse) => void;
		const batch = vi.fn(() => new Promise<AvailabilityBatchResponse>((r) => (release = r)));
		const onAnswer = vi.fn();
		const cancel = watchOtherAudio({ kitsuId: '7', mode: 'dub' }, onAnswer, batch);
		await vi.waitFor(() => expect(batch).toHaveBeenCalled());
		cancel();
		release({ cached: { '7': true }, playable_episode_counts: {} });
		await new Promise((r) => setTimeout(r, 0));
		expect(onAnswer).not.toHaveBeenCalled();
	});
});

describe('asksOtherAudio', () => {
	it('asks while the notice is up, and never for a music title', () => {
		expect(asksOtherAudio({ unavailable: true, music: false, refused: false })).toBe(true);
		expect(asksOtherAudio({ unavailable: false, music: false, refused: false })).toBe(false);
		expect(asksOtherAudio({ unavailable: true, music: true, refused: false })).toBe(false);
	});

	it('never asks for a title the provider refused by name', () => {
		// A refusal by name is about the title, not the audio: the
		// provider lists nothing named for it, in either.
		expect(asksOtherAudio({ unavailable: true, music: false, refused: true })).toBe(false);
	});
});
