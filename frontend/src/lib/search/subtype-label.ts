/**
 * The label a Kitsu subtype (`TV`, `movie`, `special`, `OVA`, `ONA`,
 * `music`) is shown with: in the search type chips, on result cards,
 * in the topbar's live results and on the detail page's eyebrow. The
 * known subtypes come from the message catalogue; one Kitsu adds later
 * shows as Kitsu spells it until it gets a message. A missing subtype
 * reads as TV, which is what Kitsu means by omitting it. Labels are
 * upper-cased, as the design sets them.
 */

import { m } from '$lib/paraglide/messages';
import { getLocale } from '$lib/paraglide/runtime';

const LABEL_BY_SUBTYPE: Readonly<Record<string, () => string>> = {
	tv: m.app_subtype_tv,
	movie: m.app_subtype_movie,
	special: m.app_subtype_special,
	ova: m.app_subtype_ova,
	ona: m.app_subtype_ona,
	music: m.app_subtype_music
};

export function subtypeLabel(subtype: string | null | undefined): string {
	const raw = subtype ?? 'TV';
	const key = raw.toLowerCase();
	const label = Object.hasOwn(LABEL_BY_SUBTYPE, key) ? LABEL_BY_SUBTYPE[key]() : raw;
	return label.toLocaleUpperCase(getLocale());
}
