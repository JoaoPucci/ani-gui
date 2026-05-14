/**
 * Shared builder for [`DownloadArgs`]: both the detail page's
 * "Download" CTA and the play page's hamburger entry hand the same
 * payload to the backend. Centralising the shape keeps the picker's
 * `episode_count` tied to Kitsu's announced total — Codex
 * P2 #3243357083 surfaced that conflating it with allmanga's
 * released-so-far count (the detail page's `episodeCap`) drops real
 * candidates via the planned-count divergence filter in
 * `pick_by_ep_count_v2`. The modal's range cap is a separate UI
 * concern, passed via DownloadConfirm's `availableEpisodes` prop.
 */
import {
	altTitlesFromKitsu,
	yearFromKitsuRef,
	type DownloadArgs,
	type KitsuAnimeRef
} from '$lib/api';

export interface BuildDownloadArgsInput {
	detail: KitsuAnimeRef;
	episode: number;
	mode: 'sub' | 'dub';
	quality: string;
	kitsuId: string;
}

export function buildDownloadArgs(input: BuildDownloadArgsInput): DownloadArgs {
	return {
		title: input.detail.canonical_title,
		episode: String(input.episode),
		mode: input.mode,
		quality: input.quality,
		episode_count: input.detail.episode_count ?? undefined,
		year: yearFromKitsuRef(input.detail) ?? undefined,
		alt_titles: altTitlesFromKitsu(input.detail),
		kitsu_id: input.kitsuId
	};
}
