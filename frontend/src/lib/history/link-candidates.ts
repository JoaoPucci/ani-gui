/**
 * The shows a guessed history row can be linked to, and what to warn
 * about before linking it to one.
 *
 * The candidates are the hits of two Kitsu searches: the row's saved
 * title, which is what the guess searched, and the words of the row's
 * provider slug (`jojos-bizarre-adventure-2000-5182` → "jojos bizarre
 * adventure 2000"), which the backend's own slug resolve searches. A
 * slug often names what the title leaves out — a year, a format — and
 * the right show can rank first there while the title's search does
 * not list it at all. The show the card shows now leads, then the two
 * lists interleaved, each show once; music videos never are one.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { isMusicSubtype, parseCour, resolveHistoryEntry } from './resolve';
import { providerOfShowId } from './show-key';

/** A show offered for the row, and whether the card shows it now. */
export interface LinkCandidate {
	ref: KitsuAnimeRef;
	current: boolean;
}

/** What a link to a show would be worth a second look for. */
export type LinkWarning = 'season' | 'episode';

/** The Kitsu-searchable words of a history row's show id: its slug's
 *  hyphenated words with the provider label and the trailing numeric
 *  id removed. Null for an id that is not slug-shaped. */
export function slugSearchTerm(showId: string): string | null {
	const slug = providerOfShowId(showId) ? showId.slice(showId.indexOf(':') + 1) : showId;
	const m = slug.match(/^(.+)-\d+$/);
	if (!m || !/^[a-z0-9-]+$/.test(m[1])) return null;
	const words = m[1].split('-').filter((w) => w.length > 0);
	return words.length > 0 ? words.join(' ') : null;
}

/** The candidates, the current show first, then the title's and the
 *  slug's hits interleaved, each show once and no music video. */
export function mergeCandidates(
	current: KitsuAnimeRef | null,
	titleHits: KitsuAnimeRef[],
	slugHits: KitsuAnimeRef[]
): LinkCandidate[] {
	const ordered: KitsuAnimeRef[] = current ? [current] : [];
	const longest = Math.max(titleHits.length, slugHits.length);
	for (let i = 0; i < longest; i++) {
		if (titleHits[i]) ordered.push(titleHits[i]);
		if (slugHits[i]) ordered.push(slugHits[i]);
	}
	const seen = new Set<string>();
	const out: LinkCandidate[] = [];
	for (const ref of ordered) {
		if (seen.has(ref.id) || isMusicSubtype(ref.subtype)) continue;
		seen.add(ref.id);
		out.push({ ref, current: ref.id === current?.id });
	}
	return out;
}

/** The season a show names: the highest any of its titles or its slug
 *  carries, 1 when none does. */
function seasonOf(ref: KitsuAnimeRef): number {
	const names = [ref.canonical_title, ...Object.values(ref.titles ?? {})];
	if (ref.slug) names.push(ref.slug.replace(/-/g, ' '));
	return Math.max(1, ...names.map((n) => parseCour(n)));
}

/** What to point out before linking `entry` to `ref`: a season its
 *  saved title names that the show does not, and an episode past the
 *  show's count — `episode`, the one the card offers, which is the
 *  number the picker names. Warnings only — the user decides. */
export function linkWarnings(
	entry: HistoryEntry,
	ref: KitsuAnimeRef,
	episode: number
): LinkWarning[] {
	const target = resolveHistoryEntry(entry, null);
	const warnings: LinkWarning[] = [];
	if (target.cour !== seasonOf(ref)) warnings.push('season');
	if (ref.episode_count !== null && episode > ref.episode_count) {
		warnings.push('episode');
	}
	return warnings;
}
