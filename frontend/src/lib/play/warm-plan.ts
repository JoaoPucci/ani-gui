import { epAirState } from '$lib/detail/episode-airing';
import type { AiringStatus } from '$lib/detail/episode-airing';
import { airedTargets, beyondPlayable } from '$lib/detail/episode-caps';

/**
 * Which episodes a page-mount warm should resolve.
 *
 * With resolution caching opted in (Config::cache_resolutions), the
 * wide fan-out earns its cost: warms mostly land as ~0.5s cache hits
 * on a revisit, and every visible tile click becomes instant. With
 * caching off — the default — every warm is a full multi-second
 * provider walk, so the plan narrows to the single episode the user
 * is most likely to play next: the strip's current+1 on the play
 * page (what keeps auto-play seamless), the Play button's target on
 * the detail page. Candidates and the narrow target are validated by
 * the caller (aired, within the playable cap); this function only
 * chooses between the two shapes.
 */
export interface WarmPlanInput {
	/** The user's Config::cache_resolutions. */
	cacheResolutions: boolean;
	/** Wide fan-out: every warmable episode the page would resolve. */
	candidates: number[];
	/** Narrow target: the one episode worth a full walk, or null
	 *  when there is none (last episode, unaired next, no cap). */
	next: number | null;
}

export function planWarm(input: WarmPlanInput): number[] {
	if (input.cacheResolutions) return input.candidates;
	return input.next === null ? [] : [input.next];
}

/** Inputs both page derivations share: the user's setting, the
 *  airing schedule and the provider's playable cap that validate
 *  every target. */
interface WarmDerivationInput {
	cacheResolutions: boolean;
	airing: AiringStatus | null;
	playableCount: number | null;
}

/** The play page's warm targets: candidates are the strip's visible
 *  tile numbers; the narrow target is current+1, whether or not its
 *  tile is on the visible strip page — warming it is what keeps
 *  auto-play seamless across the boundary. */
export function playPageWarmTargets(
	input: WarmDerivationInput & {
		visible: readonly (number | null)[];
		currentEpisode: number;
	}
): number[] {
	const warmable = (n: number) =>
		!epAirState(n, input.airing).unaired && !beyondPlayable(n, input.playableCount);
	const candidates = input.visible.filter((n): n is number => n !== null && warmable(n));
	const next = input.currentEpisode + 1;
	return planWarm({
		cacheResolutions: input.cacheResolutions,
		candidates,
		next: warmable(next) ? next : null
	});
}

/** The detail page's warm targets: candidates are the visible grid
 *  tiles (the hero target stands in until the grid loads); the
 *  narrow target is the hero Play button's episode. */
export function detailWarmTargets(
	input: WarmDerivationInput & {
		visible: readonly (number | null)[] | null;
		heroEpisode: number;
	}
): number[] {
	const playable = (targets: number[]) =>
		airedTargets(targets, input.airing).filter((n) => !beyondPlayable(n, input.playableCount));
	const candidates = playable(
		input.visible ? input.visible.filter((n): n is number => n !== null) : [input.heroEpisode]
	);
	const hero = playable([input.heroEpisode])[0] ?? null;
	return planWarm({ cacheResolutions: input.cacheResolutions, candidates, next: hero });
}
