/**
 * The order of the captions menu's entries: the viewer's languages
 * first — the language last picked on the show
 * ({@link import('./caption-choice')}), then the app's — and below a
 * divider every other track, alphabetically by the name the menu shows
 * ({@link import('./caption-label')}), in the app language's
 * collation. "Off" is not an entry here; the menu keeps it on top.
 *
 * Among your languages a track in the wanted region leads, then one
 * in the wanted script, then any other region of the language — the
 * order the start picks in ({@link import('./caption-start')}). Within a name, the plain track
 * leads its SDH twin. Ties end on the track's index, so the order does
 * not depend on the order the tracks were listed in.
 */

import { readTag, scriptOf, type Tag } from './caption-tag';

/** What the order reads of an entry. */
export interface OrderableEntry {
	index: number;
	lang: string;
	name: string;
	sdh: boolean;
}

/** What the order knows of the viewer. */
export interface CaptionPrefs {
	/** The app's locale. */
	locale: string;
	/** The show's remembered choice: a tag, or `off`. */
	remembered?: string;
}

/** The menu's two groups, each in order. */
export interface CaptionOrder<T> {
	yours: T[];
	rest: T[];
}

function collator(locale: string): Intl.Collator {
	try {
		return new Intl.Collator(locale || 'en', { sensitivity: 'base' });
	} catch {
		return new Intl.Collator('en', { sensitivity: 'base' });
	}
}

/** Where `tag` ranks among the wanted languages: three places per
 *  wanted language, in order, and within one the wanted region first,
 *  then the wanted script, then any; `null` for a tag in none. */
function rank(tag: Tag | null, wanted: readonly Tag[]): number | null {
	if (!tag) return null;
	const at = wanted.findIndex((w) => w.language === tag.language);
	if (at === -1) return null;
	const want = wanted[at];
	if (want.region && want.region === tag.region) return at * 3;
	return at * 3 + (scriptOf(want) === scriptOf(tag) ? 1 : 2);
}

export function captionOrder<T extends OrderableEntry>(
	entries: readonly T[],
	prefs: CaptionPrefs
): CaptionOrder<T> {
	const wanted: Tag[] = [];
	for (const pref of [prefs.remembered, prefs.locale]) {
		const tag = pref && pref !== 'off' ? readTag(pref) : null;
		if (tag && !wanted.some((w) => w.language === tag.language)) wanted.push(tag);
	}
	const words = collator(prefs.locale);
	const byName = (a: T, b: T) =>
		words.compare(a.name, b.name) || Number(a.sdh) - Number(b.sdh) || a.index - b.index;
	const ranked = entries.map((entry) => ({ entry, rank: rank(readTag(entry.lang), wanted) }));
	const yours = ranked
		.filter((r): r is { entry: T; rank: number } => r.rank !== null)
		.sort((a, b) => a.rank - b.rank || byName(a.entry, b.entry))
		.map((r) => r.entry);
	const rest = ranked
		.filter((r) => r.rank === null)
		.map((r) => r.entry)
		.sort(byName);
	return { yours, rest };
}
