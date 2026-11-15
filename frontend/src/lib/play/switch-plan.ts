/**
 * What the play page asks the backend about the show picked for a card
 * before "Switch to it now" (`commands/switch_episode.rs`): the card's
 * row, the episode playing, the pick's episodes with a kept position
 * (what Continue takes as left part-way, which only the renderer
 * keeps), and its last episode available — Kitsu's count capped at the
 * episodes aired, as the detail page's Continue caps it (`airedCap`),
 * Kitsu's count alone when the airing cannot be read. The answer is the
 * pick's own episode, further along than the one playing, or null for
 * the card's episode. A session no card opened asks nothing.
 */

import type { AiringStatus, KitsuAnimeRef, SwitchEpisode, SwitchEpisodeRequest } from '$lib/api';
import { airedCap } from '$lib/detail/episode-caps';

export interface SwitchPlanDeps {
	ask: (request: SwitchEpisodeRequest) => Promise<SwitchEpisode>;
	airing: (kitsuId: string) => Promise<AiringStatus>;
	kept: (kitsuId: string) => number[];
}

export async function switchEpisodeAsk(
	deps: SwitchPlanDeps,
	to: KitsuAnimeRef,
	episode: number,
	card: string | null
): Promise<number | null> {
	if (!card) return null;
	const count = to.episode_count ?? null;
	const airing = await deps.airing(to.id).catch(() => null);
	const answer = await deps.ask({
		pick: to.id,
		card,
		episode,
		episode_count: airedCap(count, airing),
		part_way: deps.kept(to.id)
	});
	return answer.episode;
}
