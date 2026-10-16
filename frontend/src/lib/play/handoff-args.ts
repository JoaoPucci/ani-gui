import type { KitsuAnimeRef, PlayArgs, VersionKind } from '$lib/api';
import { buildPlayArgs } from './play-args';

/**
 * The request a handoff — the external player or Syncplay — sends
 * for an episode: the same request the embedded play sends, so the
 * backend resolves through the same walk and, once the player has
 * started, records the watch under the same Kitsu id. The id is
 * what lets the handoff's history row be found from the detail page
 * and weighed against another provider's row; without it the
 * backend has nothing to persist the show's reverse mapping under.
 */
export function handoffArgs(input: {
	title: string;
	episode: number;
	kitsuId: string;
	config: { mode: string; quality: string };
	detail: KitsuAnimeRef | null;
	/** The version the embedded play names for the show. */
	version?: VersionKind;
}): PlayArgs {
	return buildPlayArgs({
		title: input.title,
		episode: input.episode,
		mode: input.config.mode === 'dub' ? 'dub' : 'sub',
		quality: input.config.quality || 'best',
		show: input.detail,
		kitsuId: input.kitsuId,
		version: input.version
	});
}
