/**
 * What a subtitle track reads as in the captions menu.
 *
 * A provider labels its tracks in its own words and punctuation —
 * "Portuguese (- Brazil)", "Spanish (- Latin America | SDH)" — in
 * English whatever the app's language. Every listed track carries a
 * BCP 47 tag ({@link import('./caption-tag')}), so the menu names the
 * track from that, in the app's language: "Português (Brasil)" for a
 * viewer in Portuguese. A track for the deaf and hard of hearing
 * reads as its language plus a short mark the menu translates, rather
 * than the provider's "SDH" spelled into the name. A track whose tag
 * names no language falls back to its label, cleaned of the
 * provider's punctuation, and then to its position in the list.
 *
 * Two tracks can still read the same — one language listed twice
 * under different labels — so entries that would are numbered.
 */

import { cleanLabel, isSdh } from './caption-label-clean';
import { readTag } from './caption-tag';

export { cleanLabel, isSdh };

/** What the menu reads of a listed track. */
export interface CaptionSource {
	/** The track's index in the video's TextTrackList. */
	index: number;
	lang: string;
	label: string;
}

/** A track as the menu shows it. */
export type CaptionEntry<T extends CaptionSource = CaptionSource> = T & {
	/** The name the menu shows, unique among entries with the same mark. */
	name: string;
	/** Whether the track is for the deaf and hard of hearing. */
	sdh: boolean;
};

/** How the menu words what the module cannot: a numbered duplicate
 *  and a track with nothing to name it by. */
export interface CaptionWording {
	numbered: (name: string, n: number) => string;
	untitled: (position: number) => string;
}

function displayNames(locale: string): Intl.DisplayNames {
	const options = { type: 'language', languageDisplay: 'standard', fallback: 'none' } as const;
	try {
		return new Intl.DisplayNames(locale, options);
	} catch {
		return new Intl.DisplayNames('en', options);
	}
}

/** The language `lang` names, in `locale`'s words and capitalised as
 *  a menu entry, or `null` for a tag of no language or one the locale
 *  data cannot name. */
export function languageName(lang: string, locale: string): string | null {
	if (!readTag(lang)) return null;
	const names = displayNames(locale);
	const name = names.of(lang.trim());
	if (!name) return null;
	const head = name.slice(0, 1).toLocaleUpperCase(names.resolvedOptions().locale);
	return head + name.slice(1);
}

/** Number each run of entries that read the same, until none do. */
function disambiguate<T extends CaptionSource>(
	entries: CaptionEntry<T>[],
	wording: CaptionWording
): CaptionEntry<T>[] {
	const keyOf = (e: CaptionEntry<T>) => `${e.sdh}\u0000${e.name}`;
	const counts = (list: CaptionEntry<T>[]) => {
		const groups = new Map<string, number>();
		for (const e of list) groups.set(keyOf(e), (groups.get(keyOf(e)) ?? 0) + 1);
		return groups;
	};
	let current = entries;
	// A numbered name can collide with another track's own name ("A 1"
	// listed beside two "A"s), so numbering repeats until no two read
	// the same. A chain of such collisions is never longer than the
	// list, so the bound on rounds only stops a wording that does not
	// number from looping.
	for (let round = 0, groups = counts(current); round < entries.length; round++) {
		if ([...groups.values()].every((n) => n === 1)) break;
		const seen = new Map<string, number>();
		current = current.map((e) => {
			const key = keyOf(e);
			if (groups.get(key) === 1) return e;
			const n = (seen.get(key) ?? 0) + 1;
			seen.set(key, n);
			return { ...e, name: wording.numbered(e.name, n) };
		});
		groups = counts(current);
	}
	return current;
}

/** Each track as the menu shows it, in the order given. */
export function captionEntries<T extends CaptionSource>(
	tracks: readonly T[],
	locale: string,
	wording: CaptionWording
): CaptionEntry<T>[] {
	const entries = tracks.map((track) => ({
		...track,
		name:
			languageName(track.lang, locale) ||
			cleanLabel(track.label) ||
			wording.untitled(track.index + 1),
		sdh: isSdh(track.label)
	}));
	return disambiguate(entries, wording);
}
