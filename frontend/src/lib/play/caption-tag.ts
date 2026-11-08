/**
 * A subtitle track's language tag as the start compares it — its
 * language, region and script — split from `caption-start.ts` so each
 * file stays inside the CRAP gate's per-file bar.
 */

/** A BCP 47 tag's parts as the start compares them. */
export interface Tag {
	language: string;
	region?: string;
	script?: string;
}

/** `tag` read as a language, or `null` for one that names none —
 *  `und`, an empty string, or anything that is not a tag. Three-letter
 *  codes read as the language they alias (`por` is `pt`). */
export function readTag(tag: string): Tag | null {
	try {
		const parsed = new Intl.Locale(tag.trim());
		if (!parsed.language || parsed.language === 'und') return null;
		return { language: parsed.language, region: parsed.region, script: parsed.script };
	} catch {
		return null;
	}
}

/** The script `tag` is written in: the one it names, else the one
 *  the locale data gives its language in its region (`zh-TW` writes
 *  `Hant`, `zh-CN` `Hans`). */
export function scriptOf(tag: Tag): string | undefined {
	if (tag.script) return tag.script;
	return writtenIn(tag.language, tag.region);
}

/** The script a language written in more than one is written in, in
 *  `region` or with none: Chinese in Traditional where Taiwan, Hong
 *  Kong and Macao write it and Simplified elsewhere, Serbian in Latin
 *  where Montenegro, Romania and Turkey write it and Cyrillic
 *  elsewhere; nothing for a language written in one. The backend
 *  orders an external player's tracks by the same table
 *  (`track_preference_tag.rs`), so both start on the same track. */
function writtenIn(language: string, region: string | undefined): string | undefined {
	if (language === 'zh') return ['TW', 'HK', 'MO'].includes(region ?? '') ? 'Hant' : 'Hans'; // i18n-ignore: ISO 15924 script codes, never shown
	if (language === 'sr') return ['ME', 'RO', 'TR'].includes(region ?? '') ? 'Latn' : 'Cyrl'; // i18n-ignore: ISO 15924 script codes, never shown
	return undefined;
}
