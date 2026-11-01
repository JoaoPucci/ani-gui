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
 * else none. Within a language the exact region comes first — for
 * Chinese, the script the region writes — then another region of it,
 * so a pt-BR viewer gets the pt-PT track over English when the episode
 * has no Brazilian one. A track of unknown language (`und`) is never
 * chosen by language. Off remembered starts with every track off.
 */

/** What the start reads of a listed track. */
export interface StartableTrack {
	lang: string;
	default: boolean;
}

/** A BCP 47 tag's parts as the start compares them. */
interface Tag {
	language: string;
	region?: string;
	script?: string;
}

/** `tag` read as a language, or `null` for one that names none —
 *  `und`, an empty string, or anything that is not a tag. Three-letter
 *  codes read as the language they alias (`por` is `pt`). */
function readTag(tag: string): Tag | null {
	try {
		const parsed = new Intl.Locale(tag.trim());
		if (!parsed.language || parsed.language === 'und') return null;
		return { language: parsed.language, region: parsed.region, script: parsed.script };
	} catch {
		return null;
	}
}

/** The script the language is written in in the region, as the
 *  locale data has it (`zh-TW` writes `Hant`), when there is a region. */
function regionScript(want: Tag): string | undefined {
	if (!want.region) return undefined;
	try {
		return new Intl.Locale(`${want.language}-${want.region}`).maximize().script;
	} catch {
		return undefined;
	}
}

/** Whether a track is in `want`'s own region — or writes the script
 *  `want` names, or the one `want`'s region writes. */
function sameRegion(track: Tag, want: Tag): boolean {
	if (want.region && track.region === want.region) return true;
	const script = want.script ?? regionScript(want);
	return script !== undefined && track.script === script;
}

/** The track in `wanted`'s language — its own region first — or -1. */
function inLanguage(tags: readonly (Tag | null)[], wanted: string): number {
	const want = readTag(wanted);
	if (!want) return -1;
	const precise = tags.findIndex(
		(t) => t !== null && t.language === want.language && sameRegion(t, want)
	);
	if (precise !== -1) return precise;
	return tags.findIndex((t) => t !== null && t.language === want.language);
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
