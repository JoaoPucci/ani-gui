// What the play page asks the backend about the show picked for a
// card, for "Switch to it now": the card's row, the episode playing,
// the pick's cap as Continue reads it and its episodes left part-way.

import { describe, expect, it, vi } from 'vitest';
import type { AiringStatus, KitsuAnimeRef, SwitchEpisodeRequest } from '$lib/api';
import { switchEpisodeAsk } from './switch-plan';

const pick = (episode_count: number | null) => ({ id: '8063', episode_count }) as KitsuAnimeRef;
const airing = (aired: number | null) =>
	({ aired, next_episode: null, next_airing_at: null, upcoming: [] }) as AiringStatus;

function deps(over: Partial<Parameters<typeof switchEpisodeAsk>[0]> = {}) {
	return {
		ask: vi.fn(async (r: SwitchEpisodeRequest) => ({ episode: r.episode + 5 })),
		airing: vi.fn(async () => airing(null)),
		kept: vi.fn(() => [6]),
		...over
	};
}

describe('switchEpisodeAsk', () => {
	it("asks with the card's row, the episode playing and the pick's part-way episodes", async () => {
		const d = deps();
		expect(await switchEpisodeAsk(d, pick(48), 2, 'hianime:card-1')).toBe(7);
		expect(d.ask).toHaveBeenCalledWith({
			pick: '8063',
			card: 'hianime:card-1',
			episode: 2,
			episode_count: 48,
			part_way: [6]
		});
		expect(d.kept).toHaveBeenCalledWith('8063');
	});

	it('caps the pick at the episodes aired, as Continue does for a show still airing', async () => {
		// Six aired of twelve, watched through six: Continue replays six.
		const d = deps({ airing: vi.fn(async () => airing(6)) });
		await switchEpisodeAsk(d, pick(12), 2, 'hianime:card-1');
		expect(vi.mocked(d.ask).mock.calls[0][0].episode_count).toBe(6);
		const unknown = deps({ airing: vi.fn(async () => airing(5)) });
		await switchEpisodeAsk(unknown, pick(null), 2, 'hianime:card-1');
		expect(vi.mocked(unknown.ask).mock.calls[0][0].episode_count).toBe(5);
	});

	it("keeps Kitsu's count when the airing cannot be read", async () => {
		const d = deps({ airing: vi.fn(async () => Promise.reject(new Error('down'))) });
		await switchEpisodeAsk(d, pick(12), 2, 'hianime:card-1');
		expect(vi.mocked(d.ask).mock.calls[0][0].episode_count).toBe(12);
	});

	it("answers the card's episode for a session no card opened, asking nothing", async () => {
		const d = deps();
		expect(await switchEpisodeAsk(d, pick(12), 2, null)).toBeNull();
		expect(d.ask).not.toHaveBeenCalled();
	});

	it('rejects when the backend cannot answer', async () => {
		const d = deps({ ask: vi.fn(async () => Promise.reject(new Error('down'))) });
		await expect(switchEpisodeAsk(d, pick(12), 2, 'hianime:card-1')).rejects.toThrow('down');
	});
});
