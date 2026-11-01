/**
 * Which of a session's sidecar tracks the player starts on: the one
 * attached with the `default` attribute, which the browser turns on
 * by itself when the tracks are added. Every other track starts
 * disabled and is never fetched until the captions picker shows it
 * ({@link import('./caption-pick')}).
 *
 * The start follows the viewer, not the provider: the language last
 * picked on the show ({@link import('./caption-choice')}), else the UI
 * locale's language, else the track the provider flagged default,
 * else none. Within a language the exact region comes first, then the
 * script it writes — a track's own read from its region when its tag
 * names none, so a zh-TW viewer gets `zh-HK` over `zh-CN` — then any
 * region, so a pt-BR viewer gets the pt-PT track over English when the
 * episode has no Brazilian one. A track of unknown language (`und`) is never
 * chosen by language. Off remembered starts with every track off.
 */

import { readTag, scriptOf, type Tag } from './caption-tag';

/** What the start reads of a listed track. */
export interface StartableTrack {
	lang: string;
	default: boolean;
}

/** The first track in `want`'s language that `matches`, or -1. */
function firstIn(tags: readonly (Tag | null)[], want: Tag, matches: (t: Tag) => boolean): number {
	return tags.findIndex((t) => t !== null && t.language === want.language && matches(t));
}

/** The track in `wanted`'s language, or -1: its own region first, then
 *  the script it writes, then any. */
function inLanguage(tags: readonly (Tag | null)[], wanted: string): number {
	const want = readTag(wanted);
	if (!want) return -1;
	if (want.region) {
		const exact = firstIn(tags, want, (t) => t.region === want.region);
		if (exact !== -1) return exact;
	}
	const script = scriptOf(want);
	if (script) {
		const written = firstIn(tags, want, (t) => scriptOf(t) === script);
		if (written !== -1) return written;
	}
	return firstIn(tags, want, () => true);
}

/**
 * The index of the track to start on, or `null` for none: the show's
 * `remembered` choice (a tag, or `off`), else the `locale`'s language,
 * else the provider's default.
 */
export function startingTrack(
	tracks: readonly StartableTrack[],
	locale: string,
	remembered?: string
): number | null {
	if (remembered === 'off') return null;
	const tags = tracks.map((t) => readTag(t.lang));
	for (const wanted of [remembered, locale]) {
		if (!wanted) continue;
		const at = inLanguage(tags, wanted);
		if (at !== -1) return at;
	}
	const at = tracks.findIndex((t) => t.default === true);
	return at === -1 ? null : at;
}
