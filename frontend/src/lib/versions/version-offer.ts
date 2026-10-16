/**
 * Which of a show's versions the detail page offers, and which one it
 * shows selected — split from `version-choice.ts`, which keeps the
 * remembered choice, for the per-file complexity bar.
 */

import type { VersionInfo, VersionKind } from '$lib/api';

/** The versions a page holds, as they read in the mode it shows: as
 *  given when the answer was asked in that mode; without what they say
 *  of a mode otherwise — an answer's `has_mode` is the mode it was asked
 *  about, and says nothing of another until that one is answered. */
export function versionsInMode(
	versions: VersionInfo[],
	answeredMode: 'sub' | 'dub',
	mode: 'sub' | 'dub'
): VersionInfo[] {
	if (answeredMode === mode) return versions;
	return versions.map(({ kind, show_key }) => ({ kind, show_key }));
}

/** The versions to offer a choice between: every one the answer does
 *  not say lacks the requested mode, when there are several of those,
 *  the uncensored one first and the rest in the provider's order — the
 *  provider lists a season's copies in no fixed order, and the control
 *  should not flip between seasons of one show; none otherwise, a play
 *  finding the mode in the copy that carries it. */
export function versionsToOffer(versions: VersionInfo[] | null | undefined): VersionInfo[] {
	const carrying = (versions ?? []).filter((v) => v.has_mode !== false);
	if (carrying.length < 2) return [];
	return [
		...carrying.filter((v) => v.kind === 'uncensored'),
		...carrying.filter((v) => v.kind !== 'uncensored')
	];
}

/** The version shown as selected: the remembered one when the show
 *  carries it, else the uncensored one — the one a request naming
 *  none plays — else the first. Nothing when nothing is offered. */
export function selectedVersion(
	offered: VersionInfo[],
	stored: VersionKind | undefined
): VersionKind | undefined {
	const kinds = offered.map((v) => v.kind);
	if (stored && kinds.includes(stored)) return stored;
	if (kinds.includes('uncensored')) return 'uncensored';
	return kinds[0];
}
