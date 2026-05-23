/**
 * Stub — implementation arrives in the matching green commit. Exists
 * only so the accompanying play-label.test.ts compiles for the red run.
 */
export type PlayLabelState =
	| { kind: 'watch' }
	| { kind: 'watch_again' }
	| { kind: 'episode_one' }
	| { kind: 'resume'; episode: number }
	| { kind: 'replay'; episode: number };

export function isSingleVideo(
	_subtype: string | null | undefined,
	_episodeCount: number | null | undefined,
	_status: string | null | undefined
): boolean {
	return false;
}

export function computePlayLabel(_args: {
	isSingleVideo: boolean;
	resumeEntry: { ep_no: string } | null | undefined;
	defaultEpisode: number;
}): PlayLabelState {
	return { kind: 'episode_one' };
}
