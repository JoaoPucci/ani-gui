import { altTitlesFromKitsu, yearFromKitsuRef, type KitsuAnimeRef, type PlayArgs } from '$lib/api';

/**
 * The request every play of an episode sends — the embedded play,
 * its prefetch, the Continue Watching stamp, the stale-stream
 * eviction and the handoffs. The backend keys its resolution cache
 * on these axes, so a stamp or an eviction that described the episode
 * differently from the play it follows would miss the play's row;
 * building all of them here keeps them one description.
 */
export interface PlayArgsInput {
	title: string;
	episode: number;
	mode: 'sub' | 'dub';
	quality: string;
	/** The Kitsu record the play is for, when the page has it. */
	show: KitsuAnimeRef | null;
	/** The Kitsu id to record the watch under; omitted when empty. */
	kitsuId?: string;
	/** A background warm, which must not move Continue Watching. */
	prefetch?: boolean;
}

export function buildPlayArgs(input: PlayArgsInput): PlayArgs {
	const args: PlayArgs = {
		title: input.title,
		episode: String(input.episode),
		mode: input.mode,
		quality: input.quality,
		episode_count: input.show?.episode_count ?? null,
		year: yearFromKitsuRef(input.show),
		subtype: input.show?.subtype ?? null,
		alt_titles: altTitlesFromKitsu(input.show)
	};
	if (input.prefetch === true) args.prefetch = true;
	if (input.kitsuId) args.kitsu_id = input.kitsuId;
	return args;
}
